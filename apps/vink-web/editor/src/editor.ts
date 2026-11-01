// CodeMirror 6 body editor for the Vink desk.
//
// The editor edits plain source text: prose paragraphs and poem lines separated by line breaks,
// with blank lines between stanzas. It does not interpret Markdown or produce HTML; the Rust side
// converts the text to document v1. The Rust `EditorSession` owns dirty/saving/saved/conflict
// state; this module only reports that the text changed and answers what the text is now.
//
// One EditorView lives for one open draft. Styling uses the `--vk-*` roles from vink.css, so a
// theme change is a CSS change and never reconfigures or recreates the view.

import { history, defaultKeymap, historyKeymap, insertNewline } from '@codemirror/commands';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';

export interface EditorOptions {
  /** The draft's text as loaded from the server. */
  text: string;
  /** Element ids naming and describing the text box (the visible label and hint). */
  labelledBy: string;
  describedBy: string;
  /** Id of a sticky element above the text (the toolbar); the caret is kept clear of it. */
  stickyAboveId?: string;
  /** Called after every change to the document, including IME composition updates. */
  onChange: () => void;
}

export interface EditorHandle {
  view: EditorView;
  destroyed: boolean;
}

const MOUNTED = 'vinkEditor';

export function createEditor(host: HTMLElement, options: EditorOptions): EditorHandle {
  if (host.dataset[MOUNTED]) {
    throw new Error('A Vink editor is already mounted in this host');
  }
  const handle = { destroyed: false } as EditorHandle;
  handle.view = new EditorView({
    parent: host,
    state: EditorState.create({
      doc: options.text,
      extensions: [
        history(),
        // Enter breaks the line without copying indentation: alternating poem indents stay
        // the writer's choice, as in a plain text field. Tab is left to focus navigation.
        keymap.of([{ key: 'Enter', run: insertNewline }, ...defaultKeymap, ...historyKeymap]),
        EditorView.lineWrapping,
        // Native focus after a rail control or modal may place Chrome's DOM caret at the
        // beginning. Restore the selection owned by this same view before selectionchange
        // can adopt that transient caret. Mouse selection still chooses its intended point.
        EditorView.domEventHandlers({
          focus(_event, view) {
            view.focus();
            return false;
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
          if (update.docChanged && !handle.destroyed) options.onChange();
        }),
      ],
    }),
  });
  host.dataset[MOUNTED] = 'true';
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
