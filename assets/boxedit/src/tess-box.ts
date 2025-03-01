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
  const threshold = 6;
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
      this.addEventListener("mousedown", this.handleMouseDown.bind(this), {
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
  handleMouseDown(downEvent: MouseEvent) {
    const dir = eventDir(downEvent);

    const controller = new AbortController();
    const signal = controller.signal;

    let prevX = downEvent.clientX,
      prevY = downEvent.clientY;
    window.addEventListener(
      "mousemove",
      (moveEvent) => {
        const dx = moveEvent.clientX - prevX,
          dy = moveEvent.clientY - prevY;
        prevX = moveEvent.clientX;
        prevY = moveEvent.clientY;

        this.resize(dir, dy, dx);
      },
      { signal },
    );

    window.addEventListener("mouseup", () => controller.abort(), {
      once: true,
    });
  }
  handleMouseMove(event: MouseEvent) {
    const dir = eventDir(event);
    this.style.cursor =
      cursor.get(dir) ||
      (() => {
        throw new Error(`invalid cursor ${dir.toString(2)}`);
      })();
  }
  // #endregion

  // #region keyboard-resize
  #keyboardResizeDir: Direction = Direction.Inside;
  handleKeyDown(event: KeyboardEvent) {
    if (event.shiftKey && event.key === "Delete") {
      event.preventDefault();
      this.remove();
      return true;
    }
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
    if (event.key === "Enter") {
      event.preventDefault();
      if (event.shiftKey && this.nextElementSibling instanceof TessBox) {
        this.newLine(this.nextElementSibling);
      } else {
        const targetBox = newBox(
          "",
          this.offsetLeft.toString(),
          (this.offsetTop + this.offsetHeight + 15).toString(),
          this.offsetWidth,
          this.offsetHeight,
        );
        this.after(targetBox);
        this.newLine(targetBox);
      }
      return true;
    }

    const toggleDirFocus = (dir: Direction) => {
      event.preventDefault();
      if (dir === this.#keyboardResizeDir)
        this.#keyboardResizeDir = Direction.Inside;
      else this.#keyboardResizeDir = dir;
    };

    if (event.ctrlKey)
      switch (event.key) {
        case "ArrowUp":
          toggleDirFocus(Direction.Top);
          break;
        case "ArrowDown":
          toggleDirFocus(Direction.Bottom);
          break;
        case "ArrowLeft":
          toggleDirFocus(Direction.Left);
          break;
        case "ArrowRight":
          toggleDirFocus(Direction.Right);
          break;
      }
    else if (event.shiftKey) {
      let dx = 0,
        dy = 0;
      switch (event.key) {
        case "ArrowUp":
          event.preventDefault();
          dy = -1;
          break;
        case "ArrowDown":
          event.preventDefault();
          dy = 1;
          break;
        case "ArrowLeft":
          event.preventDefault();
          dx = -1;
          break;
        case "ArrowRight":
          event.preventDefault();
          dx = 1;
          break;
      }
      this.resize(this.#keyboardResizeDir, dy, dx);
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

  newLine(targetBox: TessBox) {
    // move text to new line
    const selection = window.getSelection();
    if (selection === null) throw new Error("where selection?");
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

    this.addEventListener("dblclick", this.handleDoubleClick.bind(this));
    this.addEventListener("focus", this.handleFocus.bind(this));
    this.addEventListener("blur", this.handleBlur.bind(this));
    this.addEventListener("keydown", this.handleKeyDown.bind(this));
  }
}

customElements.define(TAG_NAME, TessBox);
