// Shared CodeMirror 6 runtime. A mounted surface owns one view and its pending UI work.
import { history, defaultKeymap, historyKeymap, insertNewline } from '@codemirror/commands';
import { Compartment, EditorSelection, EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import {
  applyHeading as mdApplyHeading,
  applyInlineFormat as mdApplyInlineFormat,
  markdownExtension,
  compactMarkdown,
} from './markdown';
import { DocumentOutline } from './outline';

export type BodyFormat = 'plain' | 'markdown';
export type EditorPreset = 'document' | 'comment';

export interface EditorOptions {
  preset?: EditorPreset;
  readOnly?: boolean;
  onComposition?: (active: boolean) => void;
  /** Exact source owned by this surface. */
  text: string;
  /** Plain source or CommonMark Live Preview. The surface controls outline capability. */
  format?: BodyFormat;
  /** Element ids naming and describing the text box (the visible label and hint). */
  labelledBy: string;
  describedBy: string;
  /** Called after every change to the document, including IME composition updates. */
  onChange: () => void;
  /** Called when the Markdown AST outline or active heading changes. */
  onOutline?: (json: string) => void;
}

export interface EditorHandle {
  view: EditorView;
  destroyed: boolean;
  preset: EditorPreset;
  format: BodyFormat;
  outline?: DocumentOutline;
  dispose: () => void;
  generation: number;
  markdownCompartment: Compartment;
  syncProgrammaticSelection: () => void;
  suppressChange: boolean;
}

const MOUNTED = 'vinkEditor';

export function createEditor(host: HTMLElement, options: EditorOptions): EditorHandle {
  if (host.dataset[MOUNTED]) {
    throw new Error('A Vink editor is already mounted in this host');
  }
  const preset = options.preset ?? 'document';
  const initialFormat: BodyFormat = options.format === 'markdown' ? 'markdown' : 'plain';
  const markdownCompartment = new Compartment();
  const handle = {
    destroyed: false,
    preset,
    format: initialFormat,
    generation: 1,
    suppressChange: false,
    markdownCompartment,
  } as EditorHandle;

  let focusSelection = EditorSelection.single(0);
  let focusEpoch = 0;
  let restoringFocus = false;
  let dispatchingRestore = false;
  let skipNextFocusRestore = false;
  let pointerDown = false;
  let scrollEnsureLeft = 0;
  let scrollFrame = 0;
  let focusFrame = 0;
  let compositionTimer = 0;
  let onChange: EditorOptions['onChange'] | undefined = options.onChange;
  let onComposition = options.onComposition;
  if (preset === 'document' && options.onOutline) {
    handle.outline = new DocumentOutline(handle, options.onOutline);
  }

  const cancelFocusRestore = () => {
    if (focusFrame) cancelAnimationFrame(focusFrame);
    focusFrame = 0;
    focusEpoch++;
    restoringFocus = false;
  };

  handle.syncProgrammaticSelection = () => {
    cancelFocusRestore();
    focusSelection = handle.view.state.selection;
    skipNextFocusRestore = true;
  };

  handle.dispose = () => {
    cancelFocusRestore();
    cancelAnimationFrame(scrollFrame);
    clearTimeout(compositionTimer);
    handle.outline?.cancelFrame();
    handle.outline = undefined;
    onChange = undefined;
    onComposition = undefined;
    handle.view.destroy();
    delete host.dataset[MOUNTED];
  };

  const ensureCaretVisible = () => {
    if (scrollFrame || scrollEnsureLeft <= 0 || handle.destroyed) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      if (handle.destroyed || !handle.view.hasFocus || restoringFocus) {
        scrollEnsureLeft = 0;
        return;
      }
      const head = handle.view.state.selection.main.head;
      const coords = handle.view.coordsAtPos(head);
      const hidden = !coords || coords.top < 0 || coords.bottom > window.innerHeight;
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
        options.readOnly ? [
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          // Commands such as insertNewline can dispatch even on a non-editable DOM node.
          EditorState.changeFilter.of(() => false),
        ] : [],
        preset === 'comment' ? compactMarkdown.of(true) : [],
        markdownCompartment.of(initialFormat === 'markdown' ? markdownExtension : []),
        // Enter breaks the line without copying indentation: alternating poem indents stay
        // the writer's choice, as in a plain text field. Tab is left to focus navigation.
        preset === 'comment' ? keymap.of([
          {key: 'Mod-b', run: () => applyInlineFormat(handle, 'bold')},
          {key: 'Mod-i', run: () => applyInlineFormat(handle, 'italic')},
        ]) : [],
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
            if (focusFrame) cancelAnimationFrame(focusFrame);
            focusFrame = requestAnimationFrame(() => {
              focusFrame = requestAnimationFrame(() => {
                focusFrame = 0;
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
              });
            });
            return false;
          },
        }),
        EditorView.domEventObservers({
          pointerdown() {
            pointerDown = true;
            cancelFocusRestore();
          },
          pointerup() {
            pointerDown = false;
          },
          pointercancel() {
            pointerDown = false;
          },
          compositionend(_event, view) {
            clearTimeout(compositionTimer);
            if (handle.format === 'markdown') {
              compositionTimer = window.setTimeout(() => {
                if (!handle.destroyed && !isComposing(handle)) {
                  view.dispatch({ selection: view.state.selection });
                  handle.outline?.refresh();
                }
              }, 30);
            }
            onComposition?.(false);
          },
          compositionstart() {
            clearTimeout(compositionTimer);
            cancelFocusRestore();
            onComposition?.(true);
          },
          blur() {
            cancelFocusRestore();
            pointerDown = false;
          },
          keydown() {
            cancelFocusRestore();
          },
        }),
        EditorView.contentAttributes.of({
          'aria-labelledby': options.labelledBy,
          'aria-describedby': options.describedBy,
          'aria-readonly': String(options.readOnly === true),
          lang: 'vi',
          spellcheck: 'false',
          autocorrect: 'off',
          autocapitalize: 'sentences',
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            cancelFocusRestore();
            if (handle.outline) handle.generation += 1;
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
            if (!handle.suppressChange) {
              onChange?.();
            }
            if (handle.format === 'markdown') {
              handle.outline?.schedule();
            }
          } else if (update.selectionSet && handle.format === 'markdown') {
            handle.outline?.selectionChanged();
          }
        }),
      ],
    }),
  });
  host.dataset[MOUNTED] = preset;
  try {
    if (initialFormat === 'markdown') handle.outline?.refresh();
  } catch (error) {
    destroyEditor(handle);
    throw error;
  }
  return handle;
}

export function destroyEditor(handle: EditorHandle): void {
  if (handle.destroyed) return;
  handle.destroyed = true;
  handle.dispose();
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

/** Replaces the document and format after an explicit, server-acknowledged conversion. */
export function applyConvertedDocument(
  handle: EditorHandle,
  format: BodyFormat,
  text: string,
): void {
  if (handle.destroyed || handle.preset !== 'document' || handle.view.state.readOnly) return;
  const next: BodyFormat = format === 'markdown' ? 'markdown' : 'plain';
  handle.format = next;
  handle.generation += 1;
  handle.suppressChange = true;
  try {
    handle.view.dispatch({
      changes: { from: 0, to: handle.view.state.doc.length, insert: text },
      selection: EditorSelection.cursor(0),
      effects: handle.markdownCompartment.reconfigure(next === 'markdown' ? markdownExtension : []),
    });
    handle.syncProgrammaticSelection();
  } finally {
    handle.suppressChange = false;
  }
  handle.outline?.refresh();
}

export function applyHeading(handle: EditorHandle, level: number): boolean {
  if (handle.destroyed || handle.preset !== 'document' || handle.format !== 'markdown' ||
      handle.view.state.readOnly || isComposing(handle)) return false;
  handle.syncProgrammaticSelection();
  const ok = mdApplyHeading(handle.view, level);
  if (ok) {
    handle.syncProgrammaticSelection();
    handle.outline?.refresh();
  }
  return ok;
}

export function applyInlineFormat(handle: EditorHandle, kind: string): boolean {
  if (handle.destroyed || handle.format !== 'markdown' || handle.view.state.readOnly || isComposing(handle)) return false;
  if ((kind === 'quote' || kind === 'list') && handle.preset === 'comment') {
    const { view } = handle;
    const { from, to } = view.state.selection.main;
    const first = view.state.doc.lineAt(from);
    const last = view.state.doc.lineAt(to > from ? to - 1 : to);
    const prefix = kind === 'quote' ? '> ' : '- ';
    const changes = [];
    for (let n = first.number; n <= last.number; n++) {
      changes.push({from: view.state.doc.line(n).from, insert: prefix});
    }
    const changeSet = view.state.changes(changes);
    const { anchor, head } = view.state.selection.main;
    view.dispatch({
      changes: changeSet,
      selection: EditorSelection.single(changeSet.mapPos(anchor, 1), changeSet.mapPos(head, 1)),
      userEvent: 'input.format',
    });
    handle.syncProgrammaticSelection();
    view.focus();
    return true;
  }
  if (kind !== 'bold' && kind !== 'italic' && kind !== 'link') return false;
  handle.syncProgrammaticSelection();
  const ok = mdApplyInlineFormat(handle.view, kind);
  if (ok) {
    handle.syncProgrammaticSelection();
    handle.outline?.refresh();
  }
  return ok;
}

export function jumpToHeading(handle: EditorHandle, from: number, expectedGeneration: number): boolean {
  const { view } = handle;
  if (
    handle.destroyed || !handle.outline || handle.format !== 'markdown' || expectedGeneration !== handle.generation ||
    isComposing(handle) || from < 0 || from > view.state.doc.length
  ) return false;
  view.dispatch({
    selection: EditorSelection.cursor(view.state.doc.lineAt(from).to),
    effects: EditorView.scrollIntoView(from, { y: 'center' }),
  });
  handle.syncProgrammaticSelection();
  view.focus();
  return true;
}
