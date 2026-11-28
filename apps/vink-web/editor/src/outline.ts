import type { EditorHandle } from './editor';
import { activeHeadingFrom, extractOutline, type OutlineSnapshot } from './markdown';

/** Owned only by a document view with an outline subscriber. */
export class DocumentOutline {
  private frame = 0;
  private last?: OutlineSnapshot;
  private json = '';

  constructor(private handle: EditorHandle, private publish: (json: string) => void) {}

  private emit(snapshot: OutlineSnapshot) {
    this.last = snapshot;
    const json = JSON.stringify(snapshot);
    if (json !== this.json) {
      this.json = json;
      this.publish(json);
    }
  }

  refresh() {
    this.cancelFrame();
    const { destroyed, format, generation, view } = this.handle;
    if (destroyed) return;
    const snapshot: OutlineSnapshot = format === 'markdown'
      ? extractOutline(view.state, generation, 8)
      : { status: 'ready', generation, items: [], title: '', activeFrom: null };
    this.emit(snapshot);
    if (snapshot.status === 'partial') this.schedule();
  }

  schedule() {
    if (!this.frame && !this.handle.destroyed) {
      this.frame = requestAnimationFrame(() => this.refresh());
    }
  }

  selectionChanged() {
    if (this.last?.generation !== this.handle.generation) {
      this.refresh();
      return;
    }
    const activeFrom = activeHeadingFrom(this.last.items, this.handle.view.state.selection.main.head);
    if (activeFrom !== this.last.activeFrom) this.emit({ ...this.last, activeFrom });
  }

  cancelFrame() {
    if (this.frame) cancelAnimationFrame(this.frame);
    this.frame = 0;
  }
}
