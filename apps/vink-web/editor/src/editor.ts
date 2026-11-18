// CodeMirror 6 body editor for the Vink desk.
//
// In `default` format, edits plain source text verbatim (prose paragraphs and poem lines).
// In `markdown` format, enables CommonMark 0.31.2 Live Preview (#256) and AST outline (#257)
// on the exact same `EditorView`. The Rust `EditorSession` owns dirty/saving/saved/conflict
// state; this module reports edits and AST outline snapshots.
//
// One EditorView lives for one open draft. Styling uses `--vk-*` roles from vink.css, so a
// theme change or format switch never recreates the view.

import { history, defaultKeymap, historyKeymap, insertNewline } from '@codemirror/commands';
import { Compartment, EditorSelection, EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import {
  activeHeadingFrom,
  applyHeading as mdApplyHeading,
  applyInlineFormat as mdApplyInlineFormat,
  extractOutline,
  jumpToHeading as mdJumpToHeading,
  markdownExtension,
  type OutlineSnapshot,
} from './markdown';

export type DocumentFormat = 'default' | 'markdown';

export interface EditorOptions {
  /** The draft's text as loaded from the server. */
  text: string;
  /** Document format controlling whether Markdown Live Preview & outline run. */
  format?: DocumentFormat;
  /** Element ids naming and describing the text box (the visible label and hint). */
  labelledBy: string;
  describedBy: string;
  /** Id of a sticky element above the text (the toolbar); the caret is kept clear of it. */
  stickyAboveId?: string;
  /** Called after every change to the document, including IME composition updates. */
  onChange: () => void;
  /** Called when the Markdown AST outline or active heading changes. */
  onOutline?: (json: string) => void;
}

export interface EditorHandle {
  view: EditorView;
  destroyed: boolean;
  format: DocumentFormat;
  generation: number;
  markdownCompartment: Compartment;
  syncProgrammaticSelection: () => void;
  scheduleOutline: (immediate?: boolean) => void;
  setSuppressedChange: (suppressed: boolean) => void;
  emitOutlineNow: () => void;
}

const MOUNTED = 'vinkEditor';

export function createEditor(host: HTMLElement, options: EditorOptions): EditorHandle {
  if (host.dataset[MOUNTED]) {
    throw new Error('A Vink editor is already mounted in this host');
  }
  const initialFormat: DocumentFormat = options.format === 'markdown' ? 'markdown' : 'default';
  const markdownCompartment = new Compartment();
  const handle = {
    destroyed: false,
    format: initialFormat,
    generation: 1,
    markdownCompartment,
  } as EditorHandle;

  let focusSelection = EditorSelection.single(0);
  let focusEpoch = 0;
  let restoringFocus = false;
  let dispatchingRestore = false;
  let skipNextFocusRestore = false;
  let pointerDown = false;
  let scrollEnsureLeft = 0;
  let scrollEnsureScheduled = false;
  let suppressChange = false;
  let outlineRaf = 0;
  let lastOutline: OutlineSnapshot | null = null;
  let lastEmittedJson = '';

  const cancelFocusRestore = () => {
    focusEpoch++;
    restoringFocus = false;
  };

  handle.syncProgrammaticSelection = () => {
    cancelFocusRestore();
    focusSelection = handle.view.state.selection;
    skipNextFocusRestore = true;
  };

  handle.setSuppressedChange = (suppressed: boolean) => {
    suppressChange = suppressed;
  };

  const emitSnapshot = (snapshot: OutlineSnapshot) => {
    lastOutline = snapshot;
    const json = JSON.stringify(snapshot);
    if (json !== lastEmittedJson) {
      lastEmittedJson = json;
      if (!handle.destroyed && options.onOutline) {
        options.onOutline(json);
      }
    }
  };

  handle.emitOutlineNow = () => {
    if (handle.destroyed || !options.onOutline) return;
    if (handle.format !== 'markdown') {
      emitSnapshot({
        status: 'ready',
        generation: handle.generation,
        items: [],
        title: '',
        activeFrom: null,
      });
      return;
    }
    emitSnapshot(extractOutline(handle.view.state, handle.generation));
  };

  handle.scheduleOutline = (immediate = false) => {
    if (handle.destroyed || !options.onOutline) return;
    if (immediate) {
      if (outlineRaf) {
        cancelAnimationFrame(outlineRaf);
        outlineRaf = 0;
      }
      handle.emitOutlineNow();
      return;
    }
    if (outlineRaf) return;
    outlineRaf = requestAnimationFrame(() => {
      outlineRaf = 0;
      handle.emitOutlineNow();
    });
  };

  const emitActiveOnly = () => {
    if (handle.destroyed || !options.onOutline || handle.format !== 'markdown') return;
    if (!lastOutline || lastOutline.generation !== handle.generation) {
      handle.scheduleOutline(true);
      return;
    }
    const nextActive = activeHeadingFrom(lastOutline.items, handle.view.state.selection.main.head);
    if (nextActive !== lastOutline.activeFrom) {
      emitSnapshot({ ...lastOutline, activeFrom: nextActive });
    }
  };

  const ensureCaretVisible = () => {
    if (scrollEnsureScheduled || scrollEnsureLeft <= 0 || handle.destroyed) return;
    scrollEnsureScheduled = true;
    requestAnimationFrame(() => {
      scrollEnsureScheduled = false;
      if (handle.destroyed || !handle.view.hasFocus || restoringFocus) {
        scrollEnsureLeft = 0;
        return;
      }
      const head = handle.view.state.selection.main.head;
      const coords = handle.view.coordsAtPos(head);
      const sticky = options.stickyAboveId ? document.getElementById(options.stickyAboveId) : null;
      const topClearance =
        sticky && getComputedStyle(sticky).position === 'sticky'
          ? Math.max(0, sticky.getBoundingClientRect().bottom)
          : 0;
      const hidden = !coords || coords.top < topClearance || coords.bottom > window.innerHeight;
      if (hidden && scrollEnsureLeft > 0) {
        scrollEnsureLeft -= 1;
        handle.view.dispatch({ effects: EditorView.scrollIntoView(head) });
        ensureCaretVisible();
      } else {
        scrollEnsureLeft = 0;
      }
    });
  };

  handle.view = new EditorView({
    parent: host,
    state: EditorState.create({
      doc: options.text,
      extensions: [
        history(),
        markdownCompartment.of(initialFormat === 'markdown' ? markdownExtension : []),
        // Enter breaks the line without copying indentation: alternating poem indents stay
        // the writer's choice, as in a plain text field. Tab is left to focus navigation.
        keymap.of([{ key: 'Enter', run: insertNewline }, ...defaultKeymap, ...historyKeymap]),
        EditorView.lineWrapping,
        // Native focus after a rail control or modal may place Chrome's DOM caret at the
        // beginning. Restore the selection owned by this same view before selectionchange
        // can adopt that transient caret. Mouse selection and explicit programmatic focus
        // still keep their intended point.
        EditorView.domEventHandlers({
          focus(_event, view) {
            if (pointerDown || skipNextFocusRestore) {
              skipNextFocusRestore = false;
              view.focus();
              return false;
            }
            const epoch = ++focusEpoch;
            restoringFocus = true;
            dispatchingRestore = true;
            try {
              view.dispatch({ selection: focusSelection });
            } finally {
              dispatchingRestore = false;
            }
            view.focus();
            requestAnimationFrame(() =>
              requestAnimationFrame(() => {
                if (handle.destroyed || epoch !== focusEpoch) return;
                if (view.hasFocus) {
                  dispatchingRestore = true;
                  try {
                    view.dispatch({ selection: focusSelection });
                  } finally {
                    dispatchingRestore = false;
                  }
                  view.focus();
                }
                restoringFocus = false;
              }),
            );
            return false;
          },
          compositionend(_event, view) {
            if (handle.format === 'markdown') {
              setTimeout(() => {
                if (!handle.destroyed && !view.composing && !view.compositionStarted) {
                  view.dispatch({ selection: view.state.selection });
                  handle.scheduleOutline(true);
                }
              }, 30);
            }
            return false;
          },
        }),
        EditorView.domEventObservers({
          mousedown() {
            pointerDown = true;
            cancelFocusRestore();
          },
          mouseup() {
            pointerDown = false;
          },
          blur() {
            pointerDown = false;
          },
          keydown() {
            cancelFocusRestore();
          },
        }),
        EditorView.contentAttributes.of({
          'aria-labelledby': options.labelledBy,
          'aria-describedby': options.describedBy,
          lang: 'vi',
          spellcheck: 'false',
          autocorrect: 'off',
          autocapitalize: 'sentences',
        }),
        EditorView.scrollMargins.of(() => {
          const sticky = options.stickyAboveId ? document.getElementById(options.stickyAboveId) : null;
          if (!sticky || getComputedStyle(sticky).position !== 'sticky') return null;
          return { top: sticky.getBoundingClientRect().height + 16 };
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            cancelFocusRestore();
            handle.generation += 1;
          } else if (
            update.selectionSet &&
            !dispatchingRestore &&
            (!update.state.selection.main.empty || update.state.selection.main.head !== 0)
          ) {
            cancelFocusRestore();
          }
          focusSelection =
            update.view.hasFocus && !restoringFocus
              ? update.state.selection
              : focusSelection.map(update.changes);
          if (
            update.view.hasFocus &&
            !restoringFocus &&
            update.transactions.some((tr) => tr.scrollIntoView)
          ) {
            scrollEnsureLeft = 2;
            ensureCaretVisible();
          }
          if (update.docChanged) {
            if (!suppressChange && !handle.destroyed) {
              options.onChange();
            }
            if (handle.format === 'markdown') {
              handle.scheduleOutline();
            }
          } else if (update.selectionSet && handle.format === 'markdown') {
            emitActiveOnly();
          }
        }),
      ],
    }),
  });
  host.dataset[MOUNTED] = 'true';
  if (initialFormat === 'markdown') {
    handle.emitOutlineNow();
  }
  return handle;
}

export function destroyEditor(handle: EditorHandle): void {
  if (handle.destroyed) return;
  handle.destroyed = true;
  const host = handle.view.dom.parentElement;
  handle.view.destroy();
  if (host) delete host.dataset[MOUNTED];
}

/** The text being written, exactly as held by the view (lines joined with `\n`). */
export function getText(handle: EditorHandle): string {
  return handle.view.state.doc.toString();
}

/** Focuses the text without scrolling the page; the selection kept by the state is restored. */
export function focusEditor(handle: EditorHandle): void {
  if (!handle.destroyed) handle.view.focus();
}

/** True while an input method is composing; a save shortcut then waits for the commit. */
export function isComposing(handle: EditorHandle): boolean {
  return handle.view.composing || handle.view.compositionStarted;
}

/** Reconfigures the Markdown Live Preview extension on the existing EditorView without remounting. */
export function setFormat(handle: EditorHandle, format: DocumentFormat): void {
  if (handle.destroyed) return;
  const next: DocumentFormat = format === 'markdown' ? 'markdown' : 'default';
  if (handle.format === next) return;
  handle.format = next;
  handle.generation += 1;
  handle.view.dispatch({
    effects: handle.markdownCompartment.reconfigure(next === 'markdown' ? markdownExtension : []),
  });
  handle.emitOutlineNow();
}

/** Replaces the document and format after an explicit, server-acknowledged conversion. */
export function applyConvertedDocument(
  handle: EditorHandle,
  format: DocumentFormat,
  text: string,
): void {
  if (handle.destroyed) return;
  const next: DocumentFormat = format === 'markdown' ? 'markdown' : 'default';
  handle.format = next;
  handle.generation += 1;
  handle.setSuppressedChange(true);
  try {
    handle.view.dispatch({
      changes: { from: 0, to: handle.view.state.doc.length, insert: text },
      selection: EditorSelection.cursor(0),
      effects: handle.markdownCompartment.reconfigure(next === 'markdown' ? markdownExtension : []),
    });
    handle.syncProgrammaticSelection();
  } finally {
    handle.setSuppressedChange(false);
  }
  handle.emitOutlineNow();
}

export function applyHeading(handle: EditorHandle, level: number): boolean {
  if (handle.destroyed || handle.format !== 'markdown') return false;
  handle.syncProgrammaticSelection();
  const ok = mdApplyHeading(handle.view, level);
  if (ok) {
    handle.syncProgrammaticSelection();
    handle.scheduleOutline(true);
  }
  return ok;
}

export function applyInlineFormat(handle: EditorHandle, kind: string): boolean {
  if (handle.destroyed || handle.format !== 'markdown') return false;
  if (kind !== 'bold' && kind !== 'italic' && kind !== 'link') return false;
  handle.syncProgrammaticSelection();
  const ok = mdApplyInlineFormat(handle.view, kind);
  if (ok) {
    handle.syncProgrammaticSelection();
    handle.scheduleOutline(true);
  }
  return ok;
}

export function jumpToHeading(handle: EditorHandle, from: number, expectedGeneration: number): boolean {
  if (handle.destroyed || handle.format !== 'markdown') return false;
  handle.syncProgrammaticSelection();
  const ok = mdJumpToHeading(handle.view, from, expectedGeneration, handle.generation);
  if (ok) {
    handle.syncProgrammaticSelection();
    handle.emitOutlineNow();
  }
  return ok;
}
