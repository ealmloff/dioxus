// This file provides the web-only Interpreter class that extends BaseInterpreter
// with all DOM mutation methods that wasm-bindgen will call directly.
//
// This replaces the sledgehammer_bindgen generated code for the web platform.

import { BaseInterpreter } from "./core";

export class Interpreter extends BaseInterpreter {
  constructor() {
    super();
  }

  pushRootById(root: number): void {
    this.pushRoot(this.nodes[root]);
  }

  appendChildrenToNode(id: number, many: number): void {
    this.appendChildren(id, many);
  }

  popRoot(): void {
    this.stack.pop();
  }

  replaceWith(id: number, n: number): void {
    const root = this.nodes[id];
    const els = this.stack.splice(this.stack.length - n);
    if ((root as any).listening) {
      this.removeAllNonBubblingListeners(root as HTMLElement);
    }
    root.replaceWith(...els);
  }

  insertAfter(id: number, n: number): void {
    const node = this.nodes[id];
    node.after(...this.stack.splice(this.stack.length - n));
  }

  insertBefore(id: number, n: number): void {
    const node = this.nodes[id];
    node.before(...this.stack.splice(this.stack.length - n));
  }

  removeNode(id: number): void {
    const node = this.nodes[id];
    if (node !== undefined) {
      if ((node as any).listening) {
        this.removeAllNonBubblingListeners(node as HTMLElement);
      }
      node.remove();
    }
  }

  createRawText(text: string): void {
    this.stack.push(document.createTextNode(text));
  }

  createTextNodeWithId(text: string, id: number): void {
    const node = document.createTextNode(text);
    this.nodes[id] = node;
    this.stack.push(node);
  }

  createPlaceholder(id: number): void {
    const node = document.createComment("placeholder");
    this.stack.push(node);
    this.nodes[id] = node;
  }

  newEventListener(eventName: string, id: number, bubbles: boolean): void {
    const node = this.nodes[id] as HTMLElement;
    if ((node as any).listening) {
      (node as any).listening += 1;
    } else {
      (node as any).listening = 1;
    }
    node.setAttribute("data-dioxus-id", `${id}`);
    this.createListener(eventName, node, bubbles);
  }

  removeEventListenerById(
    eventName: string,
    id: number,
    bubbles: boolean
  ): void {
    const node = this.nodes[id] as HTMLElement;
    (node as any).listening -= 1;
    node.removeAttribute("data-dioxus-id");
    this.removeListener(node, eventName, bubbles);
  }

  setText(id: number, text: string): void {
    this.nodes[id].textContent = text;
  }

  setAttributeById(
    id: number,
    field: string,
    value: string,
    ns: string | null
  ): void {
    const node = this.nodes[id] as HTMLElement;
    this.setAttributeInner(node, field, value, ns || "");
  }

  removeAttributeById(id: number, field: string, ns: string | null): void {
    const node = this.nodes[id] as HTMLElement;
    if (!ns) {
      switch (field) {
        case "value":
          (node as any).value = "";
          node.removeAttribute("value");
          break;
        case "checked":
          (node as any).checked = false;
          break;
        case "selected":
          (node as any).selected = false;
          break;
        case "dangerous_inner_html":
          node.innerHTML = "";
          break;
        default:
          node.removeAttribute(field);
          break;
      }
    } else if (ns === "style") {
      node.style.removeProperty(field);
    } else {
      node.removeAttributeNS(ns, field);
    }
  }

  assignIdByPath(path: Uint8Array, id: number): void {
    this.nodes[id] = this.loadChildByPath(path);
  }

  replacePlaceholderByPath(path: Uint8Array, n: number): void {
    const els = this.stack.splice(this.stack.length - n);
    const node = this.loadChildByPath(path);
    node.replaceWith(...els);
  }

  loadTemplateById(tmplId: number, index: number, id: number): void {
    const node = this.templates[tmplId][index].cloneNode(true);
    this.nodes[id] = node;
    this.stack.push(node);
  }

  private loadChildByPath(path: Uint8Array): Node {
    let node = this.stack[this.stack.length - 1] as Node;
    for (let i = 0; i < path.length; i++) {
      let end = path[i];
      for (node = node.firstChild!; end > 0; end--) {
        node = node.nextSibling!;
      }
    }
    return node;
  }
}
