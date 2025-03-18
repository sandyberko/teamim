import { ZOOM } from "./index.js";

/**
 * Clockwise from top
 */
enum Direction {
  Top = 0b1000,
  TopRight = 0b1100,
  Right = 0b0100,
  RightBottom = 0b0110,
  Bottom = 0b0010,
  BottomLeft = 0b0011,
  Left = 0b0001,
  LeftTop = 0b1001,
  Inside = 0b0000,
}

function dirToString(dir: Direction): string {
  return [Direction.Top, Direction.Right, Direction.Bottom, Direction.Left]
    .filter((d) => (dir & d) !== 0)
    .map((d) => Direction[d])
    .toString();
}

export function getDir(
  top: boolean,
  right: boolean,
  bottom: boolean,
  left: boolean,
): Direction {
  let dir = 0;
  if (top) dir |= Direction.Top;
  if (right) dir |= Direction.Right;
  if (bottom) dir |= Direction.Bottom;
  if (left) dir |= Direction.Left;
  return dir;
}

export function newBox(
  char: string,
  left: string,
  top: string,
  width: number,
  height: number,
): TessBox {
  const boxElem = document.createElement(TAG_NAME) as TessBox;
  boxElem.innerText = char;
  boxElem.style.left = left + "px";
  boxElem.style.top = top + "px";
  boxElem.style.width = width + "px";
  boxElem.style.height = height + "px";
  return boxElem;
}

function eventDir(event: MouseEvent): Direction {
  if (event.currentTarget instanceof HTMLElement === false)
    throw new Error("where target?");

  const rect = event.currentTarget.getBoundingClientRect();
  const threshold = 5;
  return getDir(
    event.y - rect.top < threshold, // top
    rect.right - event.x < threshold, // right
    rect.bottom - event.y < threshold, // bottom
    event.x - rect.left < threshold, // left
  );
}

const cursor = new Map<Direction, string>();
cursor.set(Direction.Top, "ns-resize");
cursor.set(Direction.TopRight, "ne-resize");
cursor.set(Direction.Right, "ew-resize");
cursor.set(Direction.RightBottom, "se-resize");
cursor.set(Direction.Bottom, "ns-resize");
cursor.set(Direction.BottomLeft, "sw-resize");
cursor.set(Direction.Left, "ew-resize");
cursor.set(Direction.LeftTop, "nw-resize");
cursor.set(Direction.Inside, "move");

export const TAG_NAME = "tess-box";
export class TessBox extends HTMLElement {
  handleDoubleClick(event: MouseEvent): boolean {
    if (
      event.target instanceof Element === false ||
      event.target.tagName !== "INSERT"
    )
      return false;
    const err = event.target.getAttribute("err");
    if (err === null) return false;
    event.target.replaceWith(err);
    return true;
  }

  // #region mouse-resize
  #focusController: AbortController | null = null;
  handleFocus() {
    this.style.zIndex = "2";
    if (this.#focusController === null) {
      this.#focusController = new AbortController();
      const signal = this.#focusController.signal;
      this.addEventListener("mousemove", this.handleMouseMove.bind(this), {
        signal,
      });
      this.addEventListener("pointerdown", this.handlePointerDown.bind(this), {
        signal,
      });
    }
  }
  handleBlur() {
    this.style.removeProperty("z-index");
    this.style.removeProperty("cursor");
    this.#focusController?.abort();
    this.#focusController = null;
  }
  handlePointerDown(downEvent: PointerEvent) {
    const dir = eventDir(downEvent);

    const controller = new AbortController();
    const signal = controller.signal;

    let prevX = downEvent.clientX,
      prevY = downEvent.clientY;
    window.addEventListener(
      "pointermove",
      (moveEvent) => {
        const dx = moveEvent.clientX - prevX,
          dy = moveEvent.clientY - prevY;
        prevX = moveEvent.clientX;
        prevY = moveEvent.clientY;

        this.resize(dir, dy / ZOOM, dx / ZOOM);
      },
      { signal },
    );

    window.addEventListener("pointerup", () => controller.abort(), {
      once: true,
    });
  }
  handleMouseMove(event: MouseEvent) {
    const dir = eventDir(event);
    this.style.cursor =
      cursor.get(dir) ||
      (() => {
        throw new Error(`invalid cursor ${dirToString(dir)}`);
      })();
  }
  // #endregion

  // #region keyboard
  #keyboardResize: Direction | null = null;
  handleKeyDown(event: KeyboardEvent) {
    // init keyboard resize
    if (event.altKey) {
      switch (event.code) {
        case "KeyQ": {
          event.preventDefault();
          this.#keyboardResize = Direction.Inside;
          this.#internals.states.clear();
          this.#internals.states.add("kbd-move");
          return true;
        }
        case "ArrowUp":
        case "KeyW": {
          event.preventDefault();
          this.#keyboardResize = Direction.Top;
          this.#internals.states.clear();
          this.#internals.states.add("kbd-rsz-top");
          return true;
        }
        case "ArrowLeft":
        case "KeyA": {
          event.preventDefault();
          this.#keyboardResize = Direction.Left;
          this.#internals.states.clear();
          this.#internals.states.add("kbd-rsz-left");
          return true;
        }
        case "ArrowDown":
        case "KeyS": {
          event.preventDefault();
          this.#keyboardResize = Direction.Bottom;
          this.#internals.states.clear();
          this.#internals.states.add("kbd-rsz-bottom");
          return true;
        }
        case "ArrowRight":
        case "KeyD": {
          event.preventDefault();
          this.#keyboardResize = Direction.Right;
          this.#internals.states.clear();
          this.#internals.states.add("kbd-rsz-right");
          return true;
        }
      }
    }

    // keyboard resize
    if (this.#keyboardResize !== null) {
      event.preventDefault();

      let dx = 0,
        dy = 0;
      switch (event.code) {
        case "Escape":
          this.#keyboardResize = null;
          this.#internals.states.clear();
          return true;
        case "ArrowUp":
        case "KeyW":
          dy = -1;
          break;
        case "ArrowLeft":
        case "KeyA":
          dx = -1;
          break;
        case "ArrowDown":
        case "KeyS":
          dy = 1;
          break;
        case "ArrowRight":
        case "KeyD":
          dx = 1;
          break;
      }
      this.resize(this.#keyboardResize, dy, dx);

      return true;
    }

    // delete
    if (event.shiftKey && event.key === "Delete") {
      event.preventDefault();
      this.remove();
      return true;
    }

    // new box
    if (event.key === "=") {
      event.preventDefault();
      const newBoxElemt = newBox(
        "",
        this.offsetLeft - this.offsetWidth - 10 + "",
        this.offsetTop.toString(),
        this.offsetWidth,
        this.offsetHeight,
      );
      this.after(newBoxElemt);
      newBoxElemt.focus();
      return true;
    }

    // split line
    if (event.key === "Enter") {
      event.preventDefault();
      this.splitLine(event);
      return true;
    }
  }

  resize(dir: number, dy: number, dx: number) {
    if (dir === 0b0000) dir = 0b1111;

    if ((dir & Direction.Top) != 0) {
      this.style.top = this.offsetTop + dy + "px";
      this.style.height = this.clientHeight - dy + "px";
    }
    if ((dir & Direction.Right) != 0) {
      this.style.width = this.clientWidth + dx + "px";
    }
    if ((dir & Direction.Bottom) != 0) {
      this.style.height = this.clientHeight + dy + "px";
    }
    if ((dir & Direction.Left) != 0) {
      this.style.left = this.offsetLeft + dx + "px";
      this.style.width = this.clientWidth - dx + "px";
    }
  }
  // #endregion

  // #region split line
  splitLine(event: KeyboardEvent) {
    if (event.ctrlKey) {
      if (event.shiftKey && this.previousElementSibling instanceof TessBox) {
        this.newLineBefore(this.previousElementSibling);
      } else {
        const targetBox = newBox(
          "",
          this.offsetLeft.toString(),
          (this.offsetTop - this.offsetHeight - 15).toString(),
          this.offsetWidth,
          this.offsetHeight,
        );
        this.before(targetBox);
        this.newLineBefore(targetBox);
      }
    } else {
      if (event.shiftKey && this.nextElementSibling instanceof TessBox) {
        this.newLineAfter(this.nextElementSibling);
      } else {
        const targetBox = newBox(
          "",
          this.offsetLeft.toString(),
          (this.offsetTop + this.offsetHeight + 15).toString(),
          this.offsetWidth,
          this.offsetHeight,
        );
        this.after(targetBox);
        this.newLineAfter(targetBox);
      }
    }
  }

  /**
   * move text after caret to `targetBox`
   */
  newLineAfter(targetBox: TessBox) {
    const selection = window.getSelection();
    if (selection === null) throw new Error("where selection?");
    //
    let beforeNode = selection.anchorNode;
    if (beforeNode === null) throw new Error("where selection.anchorNode?");
    if (beforeNode instanceof Text === false)
      throw new Error(
        `startContainer is not text. it is a ${beforeNode.nodeName} with "${beforeNode.textContent}"`,
      );
    let afterNode: ChildNode = beforeNode.splitText(selection.anchorOffset);
    if (beforeNode.parentNode && beforeNode.parentNode.nodeName === "DELETE") {
      const newDelete = document.createElement("delete");
      newDelete.append(afterNode);
      beforeNode = beforeNode.parentNode;
      afterNode = newDelete;
    }
    targetBox.prepend(afterNode);
    while (beforeNode.nextSibling !== null) {
      const nextNode = beforeNode.nextSibling;
      afterNode.after(nextNode);
      afterNode = nextNode;
    }
    targetBox.focus();
  }
  /*
   * move text before caret to `targetBox`
   */
  newLineBefore(targetBox: TessBox) {
    const selection = window.getSelection();
    if (selection === null) throw new Error("where selection?");
    // node before caret
    let nodeBefore = selection.anchorNode;
    if (nodeBefore === null) throw new Error("where selection.anchorNode?");
    if (nodeBefore instanceof Text === false)
      throw new Error(
        `startContainer is not text. it is a ${nodeBefore.nodeName} with "${nodeBefore.textContent}"`,
      );
    // node after caret
    let nodeAfter: Node = nodeBefore.splitText(selection.anchorOffset);
    //
    if (nodeAfter.parentNode && nodeAfter.parentNode.nodeName === "DELETE") {
      const newDelete = document.createElement("delete");
      newDelete.append(nodeBefore);
      nodeAfter = nodeAfter.parentNode;
      nodeBefore = newDelete;
    }
    targetBox.append(nodeBefore);
    let movedNodeBefore = nodeBefore as ChildNode;
    while (nodeAfter.previousSibling !== null) {
      const prevNode = nodeAfter.previousSibling;
      movedNodeBefore.before(prevNode);
      movedNodeBefore = prevNode;
    }
    targetBox.focus();
  }
  // #endregion

  connectedCallback() {
    this.contentEditable = "true";

    if (!this.hasAttribute("style")) {
      const left = this.getAttribute("left");
      const bottom = this.getAttribute("bottom");
      const right = this.getAttribute("right");
      const top = this.getAttribute("top");

      if (left !== null && bottom !== null && right !== null && top !== null) {
        this.style.left = left + "px";
        this.style.height = parseInt(bottom) - parseInt(top) + "px";
        this.style.width = parseInt(right) - parseInt(left) + "px";
        this.style.top = top + "px";
      }
    }

    this.removeAttribute("left");
    this.removeAttribute("bottom");
    this.removeAttribute("right");
    this.removeAttribute("top");
  }

  #internals: ElementInternals;
  constructor() {
    super();

    this.addEventListener("dblclick", this.handleDoubleClick.bind(this));
    this.addEventListener("focus", this.handleFocus.bind(this));
    this.addEventListener("blur", this.handleBlur.bind(this));
    this.addEventListener("keydown", this.handleKeyDown.bind(this));

    this.#internals = this.attachInternals();
  }
}

customElements.define(TAG_NAME, TessBox);
