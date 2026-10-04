/**
 * The shapes the front door reads: CDP messages from a harness or Chromium,
 * the targets Chromium lists, and the app's tabs and hooks it is started with.
 */
import type { Debugger } from 'electron';

/** A target as Chromium describes it, in /json/list (`id`) or over CDP (`targetId`). */
export interface TargetInfo {
  /** The target's id in /json/list. */
  id?: string;
  /** The target's id over CDP. */
  targetId?: string;
  /** 'page', 'iframe', 'service_worker' and the like. */
  type?: string;
  /** What the target shows. */
  url?: string;
}

/** What Input.dispatchDragEvent drags. */
export interface DragData {
  /** Files dropped from this computer. */
  files?: unknown[];
}

/** The params of a CDP command or event, as far as the front door reads them. */
export interface CdpParams {
  /** The address a navigation or a new target opens. */
  url?: string;
  /** The target a command names. */
  targetId?: string;
  /** The session a Target event or detach names. */
  sessionId?: string;
  /** The target a Target event is about. */
  targetInfo?: TargetInfo;
  /** A drag's payload, which may carry files. */
  data?: DragData;
  /** Anything else the command carries, passed through untouched. */
  [key: string]: unknown;
}

/** The result of a CDP command, as far as the front door reads it. */
export interface CdpResult {
  /** The session Target.attachToBrowserTarget opened. */
  sessionId?: string;
  /** Target.getTargets' list. */
  targetInfos?: TargetInfo[];
  /** Anything else, passed through untouched. */
  [key: string]: unknown;
}

/** One CDP message: a command, its reply, or an event. */
export interface CdpMessage {
  /** The command's id; absent on an event. */
  id?: number;
  /** The command or event name; absent on a reply. */
  method?: string;
  /** The flat session it travels on; absent for the endpoint itself. */
  sessionId?: string;
  /** The command's or event's params. */
  params?: CdpParams;
  /** A reply's result. */
  result?: CdpResult;
  /** A reply's error. */
  error?: unknown;
}

/** A harness command that passed validation: it has an id and a method. */
export interface CdpCommand extends CdpMessage {
  /** The command's id. */
  id: number;
  /** The command's name. */
  method: string;
}

/** Ends one admitted command, releasing its slot. */
export type Finish = () => void;

/** One of the app's tabs, as the front door needs it. */
export interface DoorTab {
  /** The app's tab id. */
  id: number;
  /** The tab's CDP target id, once asked. */
  targetId?: string;
  /** Settles once the first page has loaded. */
  ready?: Promise<unknown>;
  /** The tab's view, whose debugger knows its target id. */
  view: TabView;
}

/** A tab's view, down to the debugger the front door asks. */
export interface TabView {
  /** The page's contents. */
  webContents: {
    /** The page's debugger, attached by the tab's CDP setup. */
    debugger: Pick<Debugger, 'sendCommand'>;
  };
}

/** How a tab is closed. */
export interface CloseOptions {
  /** Whether closing the last tab opens a fresh one; the front door never wants that. */
  keepOne: boolean;
}

/** How `start` is told where to listen and how to reach the app. */
export interface FrontDoorOptions {
  /** The port to listen on (0 for any). */
  port: number;
  /** Chromium's own debug port, on loopback. */
  upstream: number;
  /** The address to listen on. */
  host: string;
  /** The app's tabs now. */
  tabs: () => DoorTab[];
  /** Opens a tab the protected way and answers its id. */
  createTab: (url: string, automation: boolean) => number;
  /** Closes one of the app's tabs. */
  closeTab: (id: number, options: CloseOptions) => void;
  /** Admits one automation command; answers the function that ends it. */
  beginCommand?: () => Promise<Finish> | Finish;
  /** A local client connected (+1) or left (-1). */
  clientChanged?: (delta: number) => void;
  /** The secret the server's relay presents, marking it already admitted. */
  relayToken?: string;
  /** A validation run's secret: set, the door serves only that run. */
  runToken?: string;
  /** Whether a target belongs to the validation run. */
  allowedTarget?: (id: string) => boolean;
}
