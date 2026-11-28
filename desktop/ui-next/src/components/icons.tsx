/* Phosphor is the one icon set in the app. This module keeps the old import
   names so screens do not carry a second visual language, and maps every icon
   to one weight: regular. strokeWidth is accepted and ignored on purpose. */
import type { ComponentType } from "react"
import type { IconProps as PhosphorIconProps } from "@phosphor-icons/react"
import {
  ArrowDown as PhArrowDown,
  ArrowLeft as PhArrowLeft,
  ArrowUp as PhArrowUp,
  ArrowsClockwise as PhArrowsClockwise,
  Broadcast as PhBroadcast,
  Buildings as PhBuildings,
  CaretDown as PhCaretDown,
  CaretRight as PhCaretRight,
  CaretUp as PhCaretUp,
  Check as PhCheck,
  CircleNotch as PhCircleNotch,
  ClockCounterClockwise as PhClockCounterClockwise,
  Cloud as PhCloud,
  Copy as PhCopy,
  Crown as PhCrown,
  Crosshair as PhCrosshair,
  Desktop as PhDesktop,
  DeviceMobile as PhDeviceMobile,
  DeviceTablet as PhDeviceTablet,
  DownloadSimple as PhDownloadSimple,
  GameController as PhGameController,
  Gauge as PhGauge,
  Gear as PhGear,
  Globe as PhGlobe,
  HardDrives as PhHardDrives,
  House as PhHouse,
  Info as PhInfo,
  Laptop as PhLaptop,
  Link as PhLink,
  Lock as PhLock,
  LockOpen as PhLockOpen,
  MagnifyingGlass as PhMagnifyingGlass,
  MapPin as PhMapPin,
  Microphone as PhMicrophone,
  Minus as PhMinus,
  MonitorPlay as PhMonitorPlay,
  Palette as PhPalette,
  PaintBrush as PhPaintBrush,
  Play as PhPlay,
  Plus as PhPlus,
  Plugs as PhPlugs,
  Power as PhPower,
  Printer as PhPrinter,
  Prohibit as PhProhibit,
  Pulse as PhPulse,
  ShieldCheck as PhShieldCheck,
  ShieldSlash as PhShieldSlash,
  SignIn as PhSignIn,
  SlidersHorizontal as PhSlidersHorizontal,
  Square as PhSquare,
  SquaresFour as PhSquaresFour,
  Stack as PhStack,
  Television as PhTelevision,
  TextAa as PhTextAa,
  Trash as PhTrash,
  Translate as PhTranslate,
  UserMinus as PhUserMinus,
  UsersThree as PhUsersThree,
  VideoCamera as PhVideoCamera,
  WifiHigh as PhWifiHigh,
  WifiSlash as PhWifiSlash,
  X as PhX,
} from "@phosphor-icons/react"

export type IconProps = Omit<PhosphorIconProps, "weight"> & {
  /** Kept for call-site compatibility. Phosphor uses one weight. */
  strokeWidth?: number
}

export type IconComponent = ComponentType<IconProps>

function icon(Component: ComponentType<PhosphorIconProps>) {
  return function Icon({ strokeWidth: _strokeWidth, ...props }: IconProps) {
    return <Component weight="regular" color="currentColor" {...props} />
  }
}

export const Activity = icon(PhPulse)
export const ArrowDown = icon(PhArrowDown)
export const ArrowDownToLine = icon(PhDownloadSimple)
export const ArrowLeft = icon(PhArrowLeft)
export const ArrowUp = icon(PhArrowUp)
export const Ban = icon(PhProhibit)
export const Building2 = icon(PhBuildings)
export const Cable = icon(PhPlugs)
export const Check = icon(PhCheck)
export const CheckIcon = Check
export const ChevronDown = icon(PhCaretDown)
export const ChevronDownIcon = ChevronDown
export const ChevronRight = icon(PhCaretRight)
export const ChevronRightIcon = ChevronRight
export const ChevronUpIcon = icon(PhCaretUp)
export const Cloud = icon(PhCloud)
export const Copy = icon(PhCopy)
export const Crown = icon(PhCrown)
export const Download = icon(PhDownloadSimple)
export const Gamepad2 = icon(PhGameController)
export const Gauge = icon(PhGauge)
export const Globe = icon(PhGlobe)
export const History = icon(PhClockCounterClockwise)
export const Home = icon(PhHouse)
export const Info = icon(PhInfo)
export const Languages = icon(PhTranslate)
export const Laptop = icon(PhLaptop)
export const Layers = icon(PhStack)
export const LayoutGrid = icon(PhSquaresFour)
export const Link2 = icon(PhLink)
export const Loader2 = icon(PhCircleNotch)
export const LockKeyhole = icon(PhLock)
export const LogIn = icon(PhSignIn)
export const MapPin = icon(PhMapPin)
export const Mic = icon(PhMicrophone)
export const Minus = icon(PhMinus)
export const Monitor = icon(PhDesktop)
export const MonitorPlay = icon(PhMonitorPlay)
export const Palette = icon(PhPalette)
export const PaintBrush = icon(PhPaintBrush)
export const Phone = icon(PhDeviceMobile)
export const Play = icon(PhPlay)
export const Plus = icon(PhPlus)
export const Power = icon(PhPower)
export const Printer = icon(PhPrinter)
export const Radar = icon(PhCrosshair)
export const RefreshCw = icon(PhArrowsClockwise)
export const RotateCw = icon(PhArrowsClockwise)
export const Router = icon(PhBroadcast)
export const Search = icon(PhMagnifyingGlass)
export const Server = icon(PhHardDrives)
export const Settings = icon(PhGear)
export const Settings2 = icon(PhSlidersHorizontal)
export const ShieldCheck = icon(PhShieldCheck)
export const ShieldOff = icon(PhShieldSlash)
export const Square = icon(PhSquare)
export const Tablet = icon(PhDeviceTablet)
export const Trash2 = icon(PhTrash)
export const Tv = icon(PhTelevision)
export const TextAa = icon(PhTextAa)
export const Unlock = icon(PhLockOpen)
export const UserMinus = icon(PhUserMinus)
export const Users = icon(PhUsersThree)
export const Video = icon(PhVideoCamera)
export const Wifi = icon(PhWifiHigh)
export const WifiOff = icon(PhWifiSlash)
export const X = icon(PhX)
export const XIcon = X
