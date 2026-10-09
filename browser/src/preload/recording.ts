/** Optional sandboxed recording preload; no capability is installed in the website's main world. */
import { contextBridge, ipcRenderer, webFrame } from 'electron';
import { NATIVE_RECORDING } from '../shared/native-recording.ts';
import { frameOwnerToken, type FrameOwnerLookup } from './frame-owner.ts';
import { sendRecording } from './recording-bridge.ts';
/** The sandbox owns this identity; website scripts cannot choose or read it. */
const documentId = Array.from(crypto.getRandomValues(new Uint32Array(NATIVE_RECORDING.DOCUMENT_TOKEN_WORDS))).join('-');
contextBridge.exposeInIsolatedWorld(NATIVE_RECORDING.WORLD_ID, NATIVE_RECORDING.BINDING, {
  /** A navigation creates another preload and therefore another document identity. */
  documentId,
  /** Native identity lookup stays in the agent world and is never visible to page scripts. */
  ownerToken: (selector: unknown): string => frameOwnerToken(webFrame as FrameOwnerLookup, selector),
  /** Trusted analyzer code can deliver a bounded batch for its current recording epoch. */
  emit: (epoch: unknown, payload: unknown): void =>
    sendRecording((...args) => ipcRenderer.send(...args, documentId), epoch, payload),
});
