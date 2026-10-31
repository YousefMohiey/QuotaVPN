#ifndef _TAP_H
#define _TAP_H

//
// The packet tap: one adapter at a time, frames in, frames out.
//
// The ring is a fixed pool of slots. A writer never waits: if the reader falls
// behind, the oldest slot is overwritten and counted as dropped, because a
// network tool that blocks the datapath is worse than one that misses a frame.
//

#define TAP_IOCTL_SELECT  _NDIS_CONTROL_CODE(20, METHOD_BUFFERED)
#define TAP_IOCTL_READ    _NDIS_CONTROL_CODE(21, METHOD_BUFFERED)
#define TAP_IOCTL_WRITE   _NDIS_CONTROL_CODE(22, METHOD_BUFFERED)
#define TAP_IOCTL_STATS   _NDIS_CONTROL_CODE(23, METHOD_BUFFERED)

NTSTATUS TapInitialize(VOID);
VOID     TapCleanup(VOID);

// Bind the tap to a filter instance, or to nothing with NULL.
VOID     TapSetTarget(PMS_FILTER Filter);

// Copy received frames into the ring. Called from the receive path.
VOID     TapCapture(PMS_FILTER Filter, PNET_BUFFER_LIST NetBufferLists);

// Hand one frame to the caller: ULONG length followed by the frame.
NTSTATUS TapRead(PVOID Buffer, ULONG BufferLength, PULONG Written);

// Inject one frame on the bound adapter.
NTSTATUS TapSend(PVOID Frame, ULONG Length);

// Captured, dropped, sent, queued.
VOID     TapStats(PULONG Out, PULONG OutLength);

// Remove and free frames this driver injected, so the stack never sees them.
PNET_BUFFER_LIST TapStripOwnSends(PNET_BUFFER_LIST Head);

#endif
