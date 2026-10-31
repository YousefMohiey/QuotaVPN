#include "precomp.h"
#include "tap.h"

//
// A fixed ring of slots. 512 frames of 2048 bytes is a megabyte, which is
// enough to ride out a scheduling hiccup and small enough to be a rounding
// error in a pool.
//
#define TAP_SLOT_SIZE   2048
#define TAP_SLOTS       512
#define TAP_TAG         'TAPQ'

typedef struct _TAP_SLOT
{
    ULONG Length;
    UCHAR Data[TAP_SLOT_SIZE];
} TAP_SLOT;

extern NDIS_HANDLE FilterDriverHandle;

static TAP_SLOT          *g_Slots = NULL;
static ULONG              g_Head = 0;
static ULONG              g_Tail = 0;
static NDIS_SPIN_LOCK     g_Lock;
static PMS_FILTER         g_Target = NULL;
static NDIS_HANDLE        g_NblPool = NULL;
static LONG               g_Captured = 0;
static LONG               g_Dropped = 0;
static LONG               g_Sent = 0;
static LONG               g_Queued = 0;

NTSTATUS
TapInitialize(VOID)
{
    NET_BUFFER_LIST_POOL_PARAMETERS PoolParams;

    NdisAllocateSpinLock(&g_Lock);

    g_Slots = (TAP_SLOT *)NdisAllocateMemoryWithTagPriority(
        FilterDriverHandle,
        sizeof(TAP_SLOT) * TAP_SLOTS,
        TAP_TAG,
        NormalPoolPriority);

    if (g_Slots == NULL)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    NdisZeroMemory(g_Slots, sizeof(TAP_SLOT) * TAP_SLOTS);

    NdisZeroMemory(&PoolParams, sizeof(PoolParams));
    PoolParams.Header.Type = NDIS_OBJECT_TYPE_DEFAULT;
    PoolParams.Header.Revision = NET_BUFFER_LIST_POOL_PARAMETERS_REVISION_1;
    PoolParams.Header.Size = NDIS_SIZEOF_NET_BUFFER_LIST_POOL_PARAMETERS_REVISION_1;
    PoolParams.ProtocolId = NDIS_PROTOCOL_ID_DEFAULT;
    PoolParams.fAllocateNetBuffer = TRUE;
    PoolParams.ContextSize = sizeof(PVOID);
    PoolParams.PoolTag = TAP_TAG;

    g_NblPool = NdisAllocateNetBufferListPool(FilterDriverHandle, &PoolParams);
    if (g_NblPool == NULL)
    {
        NdisFreeMemory(g_Slots, 0, 0);
        g_Slots = NULL;
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    return STATUS_SUCCESS;
}

VOID
TapCleanup(VOID)
{
    if (g_NblPool != NULL)
    {
        NdisFreeNetBufferListPool(g_NblPool);
        g_NblPool = NULL;
    }

    if (g_Slots != NULL)
    {
        NdisFreeMemory(g_Slots, 0, 0);
        g_Slots = NULL;
    }
}

VOID
TapSetTarget(PMS_FILTER Filter)
{
    NdisAcquireSpinLock(&g_Lock);
    g_Target = Filter;
    g_Head = 0;
    g_Tail = 0;
    g_Queued = 0;
    NdisReleaseSpinLock(&g_Lock);
}

VOID
TapCapture(PMS_FILTER Filter, PNET_BUFFER_LIST NetBufferLists)
{
    PNET_BUFFER_LIST nbl;
    PNET_BUFFER      nb;
    ULONG            length;
    ULONG            next;
    ULONG            copy;
    PVOID            data;
    TAP_SLOT        *slot;

    if (g_Slots == NULL || Filter == NULL || Filter != g_Target)
    {
        return;
    }

    for (nbl = NetBufferLists; nbl != NULL; nbl = NET_BUFFER_LIST_NEXT_NBL(nbl))
    {
        nb = NET_BUFFER_LIST_FIRST_NB(nbl);
        if (nb == NULL)
        {
            continue;
        }

        length = NET_BUFFER_DATA_LENGTH(nb);
        if (length == 0)
        {
            continue;
        }

        NdisAcquireSpinLock(&g_Lock);

        next = (g_Head + 1) % TAP_SLOTS;
        if (next == g_Tail)
        {
            g_Dropped++;
            NdisReleaseSpinLock(&g_Lock);
            continue;
        }

        slot = &g_Slots[g_Head];
        copy = (length > TAP_SLOT_SIZE) ? TAP_SLOT_SIZE : length;

        data = NdisGetDataBuffer(nb, copy, slot->Data, 1, 0);
        if (data == NULL)
        {
            g_Dropped++;
            NdisReleaseSpinLock(&g_Lock);
            continue;
        }

        if (data != slot->Data)
        {
            NdisMoveMemory(slot->Data, data, copy);
        }

        slot->Length = copy;
        g_Head = next;
        g_Captured++;
        g_Queued++;

        NdisReleaseSpinLock(&g_Lock);
    }
}

NTSTATUS
TapRead(PVOID Buffer, ULONG BufferLength, PULONG Written)
{
    TAP_SLOT *slot;
    ULONG     length;

    *Written = 0;

    if (g_Slots == NULL)
    {
        return STATUS_DEVICE_NOT_READY;
    }

    NdisAcquireSpinLock(&g_Lock);

    if (g_Tail == g_Head)
    {
        NdisReleaseSpinLock(&g_Lock);
        return STATUS_NO_MORE_ENTRIES;
    }

    slot = &g_Slots[g_Tail];
    length = slot->Length;

    if (length + sizeof(ULONG) > BufferLength)
    {
        g_Tail = (g_Tail + 1) % TAP_SLOTS;
        g_Queued--;
        g_Dropped++;
        NdisReleaseSpinLock(&g_Lock);
        return STATUS_BUFFER_TOO_SMALL;
    }

    *(PULONG)Buffer = length;
    NdisMoveMemory((PUCHAR)Buffer + sizeof(ULONG), slot->Data, length);

    g_Tail = (g_Tail + 1) % TAP_SLOTS;
    g_Queued--;

    NdisReleaseSpinLock(&g_Lock);

    *Written = length + sizeof(ULONG);
    return STATUS_SUCCESS;
}

NTSTATUS
TapSend(PVOID Frame, ULONG Length)
{
    PVOID            buffer;
    PMDL             mdl;
    PNET_BUFFER_LIST nbl;
    PVOID           *context;

    if (g_Target == NULL || g_NblPool == NULL)
    {
        return STATUS_DEVICE_NOT_READY;
    }

    if (Frame == NULL || Length == 0 || Length > TAP_SLOT_SIZE)
    {
        return STATUS_INVALID_PARAMETER;
    }

    buffer = NdisAllocateMemoryWithTagPriority(FilterDriverHandle, Length, TAP_TAG, NormalPoolPriority);
    if (buffer == NULL)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    NdisMoveMemory(buffer, Frame, Length);

    mdl = IoAllocateMdl(buffer, Length, FALSE, FALSE, NULL);
    if (mdl == NULL)
    {
        NdisFreeMemory(buffer, 0, 0);
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    MmBuildMdlForNonPagedPool(mdl);

    nbl = NdisAllocateNetBufferAndNetBufferList(g_NblPool, sizeof(PVOID), 0, mdl, 0, Length);
    if (nbl == NULL)
    {
        IoFreeMdl(mdl);
        NdisFreeMemory(buffer, 0, 0);
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    // The context holds the buffer so the completion path can free it.
    context = (PVOID *)NET_BUFFER_LIST_CONTEXT_DATA_START(nbl);
    *context = buffer;

    g_Sent++;

    NdisFSendNetBufferLists(g_Target->FilterHandle, nbl, 0, 0);

    return STATUS_SUCCESS;
}

VOID
TapStats(PULONG Out, PULONG OutLength)
{
    Out[0] = (ULONG)g_Captured;
    Out[1] = (ULONG)g_Dropped;
    Out[2] = (ULONG)g_Sent;
    Out[3] = (ULONG)g_Queued;
    *OutLength = 4 * sizeof(ULONG);
}

PNET_BUFFER_LIST
TapStripOwnSends(PNET_BUFFER_LIST Head)
{
    PNET_BUFFER_LIST current = Head;
    PNET_BUFFER_LIST previous = NULL;
    PNET_BUFFER_LIST next;
    PVOID           *context;
    PVOID            buffer;
    PMDL             mdl;

    while (current != NULL)
    {
        next = NET_BUFFER_LIST_NEXT_NBL(current);
        context = (PVOID *)NET_BUFFER_LIST_CONTEXT_DATA_START(current);

        if (context != NULL && *context != NULL)
        {
            buffer = *context;
            mdl = NET_BUFFER_FIRST_MDL(NET_BUFFER_LIST_FIRST_NB(current));

            if (previous != NULL)
            {
                NET_BUFFER_LIST_NEXT_NBL(previous) = next;
            }
            else
            {
                Head = next;
            }

            NET_BUFFER_LIST_NEXT_NBL(current) = NULL;
            NdisFreeNetBufferList(current);

            if (mdl != NULL)
            {
                IoFreeMdl(mdl);
            }

            NdisFreeMemory(buffer, 0, 0);
        }
        else
        {
            previous = current;
        }

        current = next;
    }

    return Head;
}
