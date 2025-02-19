import { ZOOM } from './index.js'

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

export function getDir(top: boolean, right: boolean, bottom: boolean, left: boolean): Direction {
    let dir = 0;
    if (top) dir |= Direction.Top;
    if (right) dir |= Direction.Right;
    if (bottom) dir |= Direction.Bottom;
    if (left) dir |= Direction.Left;
    return dir;
}

export function newBox(char: string, left: string, top: string, width: number, height: number): TessBox {
    const boxElem = document.createElement(TAG_NAME) as TessBox;
    boxElem.innerText = char;
    boxElem.style.left = left + 'px';
    boxElem.style.top = top + 'px';
    boxElem.style.width = width + 'px';
    boxElem.style.height = height + 'px';
    return boxElem;
}

function eventDir(event: MouseEvent): Direction {
    if (event.currentTarget instanceof HTMLElement === false) throw new Error("where target?");

    const rect = event.currentTarget.getBoundingClientRect();
    return getDir(
        event.y - rect.top < 4, // top
        rect.right - event.x < 4, // right
        rect.bottom - event.y < 4, // bottom
        event.x - rect.left < 4, // left
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

export const TAG_NAME = 'tess-box';
export class TessBox extends HTMLElement {

    handleDoubleClick(event: MouseEvent): boolean {
        if (event.target instanceof Element === false || event.target.tagName !== "INSERT") return false;
        const err = event.target.getAttribute("err");
        if (err === null) return false;
        event.target.replaceWith(err);
        return true;
    }

    // #region mouse-resize
    #focusController: AbortController | null = null;
    handleFocus(_: FocusEvent) {
        this.style.zIndex = "2";
        if (this.#focusController === null) {
            this.#focusController = new AbortController();
            const signal = this.#focusController.signal;
            this.addEventListener("mousemove", this.handleMouseMove.bind(this), { signal });
            this.addEventListener("mousedown", this.handleMouseDown.bind(this), { signal });
        }
    }
    handleBlur(_: FocusEvent) {
        this.style.zIndex = "0";
        this.style.cursor = "default";
        this.#focusController?.abort();
        this.#focusController = null;
    }
    handleMouseDown(downEvent: MouseEvent) {
        downEvent.preventDefault();

        const dir = eventDir(downEvent);

        const controller = new AbortController();
        const signal = controller.signal;

        let prevX = downEvent.clientX/ZOOM, prevY = downEvent.clientY/ZOOM;
        this.addEventListener("mousemove", (moveEvent) => {
            const dx = moveEvent.clientX/ZOOM - prevX, dy = moveEvent.clientY/ZOOM - prevY;
            prevX = moveEvent.clientX/ZOOM;
            prevY = moveEvent.clientY/ZOOM;

            this.resize(dir, dy, dx);
        }, { signal });

        this.addEventListener("mouseup", () => controller.abort(), { signal });
        this.addEventListener("mouseleave", () => controller.abort(), { signal });
    }
    handleMouseMove(event: MouseEvent) {
        const dir = eventDir(event);
        this.style.cursor = cursor.get(dir) || (() => { throw new Error(`invalid cursor ${dir.toString(2)}`); })();
    }
    // #endregion

    // #region keyboard-resize
    #keyboardResizeDir: Direction = Direction.Inside;
    handleKeyDown(event: KeyboardEvent) {
        if (event.shiftKey && event.key === "Delete") {
            event.preventDefault();
            this.remove();
            return true;
        };
        if (event.key === "=") {
            event.preventDefault();
            const newBoxElemt = newBox(
                "",
                this.offsetLeft - this.offsetWidth - 10 + "",
                this.offsetTop.toString(), this.offsetWidth,
                this.offsetHeight
            );
            this.after(newBoxElemt);
            newBoxElemt.focus();
            return true;
        }

        const toggleDirFocus = (dir: Direction) => {
            event.preventDefault();
            if (dir === this.#keyboardResizeDir) this.#keyboardResizeDir = Direction.Inside;
            else this.#keyboardResizeDir = dir;
        };

        if (event.ctrlKey) switch (event.key) {
            case "ArrowUp": toggleDirFocus(Direction.Top); break;
            case "ArrowDown": toggleDirFocus(Direction.Bottom); break;
            case "ArrowLeft": toggleDirFocus(Direction.Left); break;
            case "ArrowRight": toggleDirFocus(Direction.Right); break;
        } else if (event.shiftKey) {
            let dx = 0, dy = 0;
            switch (event.key) {
                case "ArrowUp": event.preventDefault(); dy = -1; break;
                case "ArrowDown": event.preventDefault(); dy = 1; break;
                case "ArrowLeft": event.preventDefault(); dx = -1; break;
                case "ArrowRight": event.preventDefault(); dx = 1; break;
            }
            this.resize(this.#keyboardResizeDir, dy, dx);
        }
    }
    resize(dir: number, dy: number, dx: number) {
        if (dir === 0b0000) dir = 0b1111;

        if ((dir & Direction.Top) != 0) {
            this.style.top = this.offsetTop + dy + 'px';
            this.style.height = this.clientHeight - dy + 'px';
        }
        if ((dir & Direction.Right) != 0) {
            this.style.width = this.clientWidth + dx + 'px';
        }
        if ((dir & Direction.Bottom) != 0) {
            this.style.height = this.clientHeight + dy + 'px';
        }
        if ((dir & Direction.Left) != 0) {
            this.style.left = this.offsetLeft + dx + 'px';
            this.style.width = this.clientWidth - dx + 'px';
        }
    }
    // #endregion

    connectedCallback() {
        this.tabIndex = 0;
        this.contentEditable = "true";

        const left = this.getAttribute("left");
        const bottom = this.getAttribute("bottom");
        const right = this.getAttribute("right");
        const top = this.getAttribute("top");

        if (left !== null && bottom !== null && right !== null && top !== null) {
            this.style.left = left + 'px';
            this.style.height = parseInt(bottom) - parseInt(top) + 'px';
            this.style.width = parseInt(right) - parseInt(left) + 'px';
            this.style.top = top + 'px';
        }

        this.addEventListener("dblclick", this.handleDoubleClick.bind(this));
        this.addEventListener("focus", this.handleFocus.bind(this));
        this.addEventListener("blur", this.handleBlur.bind(this));
        this.addEventListener("keydown", this.handleKeyDown.bind(this));
    }

    constructor() {
        super();
    }
}

customElements.define(TAG_NAME, TessBox);