"use strict";

let image: HTMLImageElement | null = null;

const main = document.getElementById("main");
if (main instanceof HTMLElement === false) throw new Error("where main?");

const boxContainer = document.getElementById("box-container");
if (boxContainer instanceof HTMLElement) { } else { throw new Error("where box container?"); }

const imageInput = document.getElementById("image-input");
if (imageInput instanceof HTMLInputElement === false) throw new Error("where image input?");

const imageSaveButton = document.getElementById("save-image");
if (imageSaveButton instanceof HTMLAnchorElement === false) throw new Error("where save button?");


imageInput.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    const imageData = input.files![0];
    const url = URL.createObjectURL(imageData);
    if (image === null) {
        image = new Image();
        document.getElementById("main")!.appendChild(image);
    }
    image.onload = (event) => {
        if (event.target instanceof HTMLImageElement === false) throw new Error("where image?");
        boxContainer!.style.width = event.target.width + 'px';
        boxContainer!.style.height = event.target.height + 'px';
    }

    image.src = url;
});

document.getElementById("box-input")!.addEventListener("change", (event) => {
    const file = event.target as HTMLInputElement;
    const reader = new FileReader();
    reader.readAsText(file.files![0], 'UTF-8');
    reader.onload = function (event) {
        const text = event.target?.result as string;
        renderBoxes(text);
    }
});

// #region Render boxes
function renderBoxes(text: string) {
    const boxContainer = document.getElementById("box-container");
    if (boxContainer instanceof HTMLElement === false) throw new Error("where main?");
    boxContainer.innerHTML = "";
    for (const line of text.split("\n")) {
        const box = line.trim();
        if (box === "") continue;
        const [char, left, bottom, right, top] = box.split(" ");
        const width = parseInt(right) - parseInt(left);
        const height = parseInt(bottom) - parseInt(top);
        const boxElem = newBox(char, left, top, width, height);
        boxContainer.appendChild(boxElem);
    };
}

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

function newBox(char: string, left: string, top: string, width: number, height: number) {
    const boxElem = document.createElement("input");
    boxElem.classList.add("box");
    boxElem.type = "text";
    boxElem.value = char;
    // boxElem.maxLength = 1;
    // boxElem.minLength = 1;
    boxElem.style.left = left + 'px';
    boxElem.style.top = top + 'px';
    boxElem.style.width = width + 'px';
    boxElem.style.height = height + 'px';
    return boxElem;
}

function getDir(top: boolean, right: boolean, bottom: boolean, left: boolean): Direction {
    let dir = 0;
    if (top) dir |= Direction.Top;
    if (right) dir |= Direction.Right;
    if (bottom) dir |= Direction.Bottom;
    if (left) dir |= Direction.Left;
    return dir;
}

function eventDir(event: MouseEvent): Direction {
    if (event.target instanceof HTMLElement === false) throw new Error("where target?");

    const rect = event.target.getBoundingClientRect();
    return getDir(
        event.y - rect.top < 4, // top
        rect.right - event.x < 4, // right
        rect.bottom - event.y < 4, // bottom
        event.x - rect.left < 4, // left
    );
}

const hebrewLetterRegex = /[\u05d0-\u05ea]/;
function setupKeyboardResize(elem: HTMLInputElement) {
    let keyboardResizeDir: Direction = Direction.Inside;

    elem.addEventListener("keydown", (event) => {
        if (event.key === "Delete") {
            event.preventDefault();
            elem.remove();
            return true;
        };
        if (event.key === "=") {
            event.preventDefault();
            const newBoxElemt = newBox(
                "",
                elem.offsetLeft - elem.offsetWidth - 10 + "",
                elem.offsetTop.toString(), elem.offsetWidth,
                elem.offsetHeight
            );
            elem.after(newBoxElemt);
            newBoxElemt.focus();
            return true;
        }

        const toggleDirFocus = (dir: Direction) => {
            event.preventDefault();
            if (dir === keyboardResizeDir) keyboardResizeDir = Direction.Inside;
            else keyboardResizeDir = dir;
        };

        if (event.ctrlKey) switch (event.key) {
            case "ArrowUp": toggleDirFocus(Direction.Top); break;
            case "ArrowDown": toggleDirFocus(Direction.Bottom); break;
            case "ArrowLeft": toggleDirFocus(Direction.Left); break;
            case "ArrowRight": toggleDirFocus(Direction.Right); break;
        }
        else {
            let dx = 0, dy = 0;
            switch (event.key) {
                case "ArrowUp": event.preventDefault(); dy = -1; break;
                case "ArrowDown": event.preventDefault(); dy = 1; break;
                case "ArrowLeft": event.preventDefault(); dx = -1; break;
                case "ArrowRight": event.preventDefault(); dx = 1; break;
            }
            resizeElem(elem, keyboardResizeDir, dy, dx);
        }
    });
    elem.addEventListener("beforeinput", (event) => {
        if (event.data && event.data.match(hebrewLetterRegex) === null) {
            event.preventDefault();
            alert("אותיות עבריות בלבד. האם המקלדת על עברית?")
        }
    });
}
// #endregion

// #region Resize
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

function handleMouseMove(event: MouseEvent) {
    if (event.target instanceof HTMLInputElement === false) return false;
    const dir = eventDir(event);
    event.target.style.cursor = cursor.get(dir) || (() => { throw new Error(`invalid cursor ${dir.toString(2)}`); })();
}

function handleMouseDown(downEvent: MouseEvent) {
    if (downEvent.target instanceof HTMLInputElement === false) return false;
    downEvent.preventDefault();

    const elem = downEvent.target;
    const dir = eventDir(downEvent);

    const controller = new AbortController();
    const signal = controller.signal;

    let prevX = downEvent.clientX, prevY = downEvent.clientY;
    boxContainer!.addEventListener("mousemove", (moveEvent) => {
        const dx = moveEvent.clientX - prevX, dy = moveEvent.clientY - prevY;
        prevX = moveEvent.clientX;
        prevY = moveEvent.clientY;

        resizeElem(elem, dir, dy, dx);
    }, { signal });

    boxContainer!.addEventListener("mouseup", () => controller.abort(), { signal });
}

{
    boxContainer.addEventListener("focusin", (event) => {
        if (event.target instanceof HTMLInputElement === false) return false;
        event.target.style.zIndex = "2";
        event.target.select();
        event.target.addEventListener("mousemove", handleMouseMove);
        event.target.addEventListener("mousedown", handleMouseDown);
    });
    boxContainer.addEventListener("focusout", (event) => {
        if (event.target instanceof HTMLInputElement === false) return false;
        event.target.style.zIndex = "0";
        event.target.removeEventListener("mousemove", handleMouseMove);
        event.target.removeEventListener("mousedown", handleMouseDown);
    });
}
// #endregion

// #region Training Boxes
{
    // lstm box format
    function renderTrainingBoxes(text: string) {
        if (!image) throw new Error("where image?");
        const imgHeight = image.height;

        const boxContainer = document.getElementById("box-container");
        if (boxContainer instanceof HTMLElement === false) throw new Error("where main?");
        boxContainer.innerHTML = "";
        let lastBox: HTMLInputElement | null = null;
        for (const line of text.split("\n")) {
            if (line === "") continue;
            const char = line.substring(0, 1);
            if (char === "\t") {
                lastBox = null;
            } else if (lastBox) {
                lastBox.value = char + lastBox.value;
                continue;
            } else {
                const [left, blBottom, right, blTop] = line.substring(2).split(" ");
                const top = imgHeight - parseInt(blTop);
                const bottom = imgHeight - parseInt(blBottom);
                const width = parseInt(right) - parseInt(left);
                const height = bottom - top;
                lastBox = newBox(char, left, top.toString(), width, height);
                boxContainer.appendChild(lastBox);
            }
        };
    }
    document.getElementById("training-input")!.addEventListener("change", (event) => {
        const file = event.target as HTMLInputElement;
        const reader = new FileReader();
        reader.readAsText(file.files![0], 'UTF-8');
        reader.onload = function (event) {
            const text = event.target?.result as string;
            renderTrainingBoxes(text);
        }
    });

    let outputFile: FileSystemFileHandle | null = null;
    document.getElementById("training-save")!.addEventListener("click", async (event) => {
        // create a new handle
        if (!outputFile) outputFile = await window.showSaveFilePicker();

        // create a FileSystemWritableFileStream to write to
        const writableStream = await outputFile.createWritable();

        if (!image) throw new Error("where image?");
        const imgHeight = image.height;

        // write our file
        for (const box of boxContainer.childNodes) {
            if (box instanceof HTMLInputElement === false) continue;

            // lstm box format
            const left = box.offsetLeft;
            const bottom = imgHeight - box.offsetTop - box.offsetHeight;
            const right = box.offsetLeft + box.offsetWidth;
            const top = imgHeight - box.offsetTop;

            for (const char of Array.from(box.value).reverse()) {
                await writableStream.write(`${char} ${left} ${bottom} ${right} ${top} 0\n`);
            }
            await writableStream.write(`\t ${left} ${bottom} ${right} ${top} 0\n`);
        }

        // close the file and write the contents to disk.
        await writableStream.close();
    });
}
// #endregion

// Visibility
const viewAttr = "data-view";
const keyMap: Record<string, (active: boolean) => void> = {
    // image
    "F1": (show) => { image && (image.style.opacity = show ? "1" : "0"); },
    // box
    "F2": (show) => { boxContainer.style.opacity = show ? "1" : "0"; },
    // box + text
    "F3": (show) => { show ? boxContainer.removeAttribute(viewAttr) : boxContainer.setAttribute(viewAttr, "text-only"); },
    // solo box
    "F4": (show) => { show ? boxContainer.removeAttribute(viewAttr) : boxContainer.setAttribute(viewAttr, "solo"); },
    // diff
    "F5": (show) => { show && diff(); }
}
document.addEventListener("keydown", (event) => {
    if (event.key in keyMap) {
        event.preventDefault();
        keyMap[event.key](false);
    }
});
document.addEventListener("keyup", (event) => {
    if (event.shiftKey) return;
    if (event.key in keyMap) {
        event.preventDefault();
        keyMap[event.key](true);
    }
});

{
    let outputFile: FileSystemFileHandle | null = null;
    document.getElementById("save-box")!.addEventListener("click", async (event) => {
        // create a new handle
        if (!outputFile) outputFile = await window.showSaveFilePicker();

        // create a FileSystemWritableFileStream to write to
        const writableStream = await outputFile.createWritable();

        if (!image) throw new Error("where image?");
        const imgHeight = image.height;

        // write our file
        for (const box of boxContainer.childNodes) {
            if (box instanceof HTMLInputElement === false) continue;

            // top-left custom box format
            // const char = box.value;
            // const left = box.offsetLeft;
            // const bottom = box.offsetTop + box.offsetHeight;
            // const right = box.offsetLeft + box.offsetWidth;
            // const top = box.offsetTop;

            // await writableStream.write(`${char} ${left} ${bottom} ${right} ${top} 0\n`);

            // lstm box format
            const left = box.offsetLeft;
            const bottom = imgHeight - box.offsetTop - box.offsetHeight;
            const right = box.offsetLeft + box.offsetWidth;
            const top = imgHeight - box.offsetTop;

            for (const char of Array.from(box.value).reverse()) {
                await writableStream.write(`${char} ${left} ${bottom} ${right} ${top} 0\n`);
            }
            await writableStream.write(`\t ${left} ${bottom} ${right} ${top} 0\n`);
        }

        // close the file and write the contents to disk.
        await writableStream.close();
    });
}

function getBoxes() {
    if (boxContainer instanceof HTMLElement === false) throw new Error("where main?");
    return Array.from(boxContainer.childNodes)
        .map((box) => {
            if (box instanceof HTMLInputElement === false) throw new Error("invalid box");
            const char = box.value;
            const left = box.offsetLeft;
            const bottom = box.offsetTop + box.offsetHeight;
            const right = box.offsetLeft + box.offsetWidth;
            const top = box.offsetTop;
            return `${char} ${left} ${bottom} ${right} ${top} 0`;
        })
        .join("\n");
}

function resizeElem(elem: HTMLInputElement, dir: number, dy: number, dx: number) {
    if (dir === 0b0000) dir = 0b1111;

    if ((dir & Direction.Top) != 0) {
        elem.style.top = elem.offsetTop + dy + 'px';
        elem.style.height = elem.clientHeight - dy + 'px';
    }
    if ((dir & Direction.Right) != 0) {
        elem.style.width = elem.clientWidth + dx + 'px';
    }
    if ((dir & Direction.Bottom) != 0) {
        elem.style.height = elem.clientHeight + dy + 'px';
    }
    if ((dir & Direction.Left) != 0) {
        elem.style.left = elem.offsetLeft + dx + 'px';
        elem.style.width = elem.clientWidth - dx + 'px';
    }
}

document.getElementById("recognize")!.addEventListener("click", async (event) => {
    // get image from input
    const imageData = imageInput.files![0];
    const response = await fetch("/recognize", {
        method: "POST",
        body: imageData
    });
    if (response.status === 400) {
        imageInput.focus();
        alert("לא נבחרה תמונה");
    }
    if (!response.ok) { throw new Error("Failed to recognize"); }
    const text = await response.text();
    renderBoxes(text);
});

document.getElementById("render-teamim")!.addEventListener("click", async (event) => {
    const formData = new FormData();
    formData.append("image", imageInput.files![0]);
    formData.append("boxes", getBoxes());
    const response = await fetch("/renderTeamim", {
        method: "POST",
        body: formData,
    });
    // parse error
    if (response.status === 400) {
        const error = await response.json();
        if (typeof error === "string" && error === "Not found") {
            alert("טקסט לא נמצא, נסה לתקן טעויות זיהוי ולהריץ שוב");
        } else {
            const { boxNumber, expected } = error;
            const box = boxContainer.childNodes[boxNumber];
            if (box instanceof HTMLInputElement === false) throw new Error("where box?");
            box.focus();
            box.select();
            // blink box
            box.animate([{ backgroundColor: "red" }, { backgroundColor: "white" }], {
                duration: 1000,
                iterations: 5,
            });
            box.setCustomValidity(expected);
            box.reportValidity();
        }
    } else if (response.status !== 200) {
        throw new Error("Failed to recognize");
    } else {
        if (image === null) throw new Error("where image?");
        const img = await response.blob();
        const url = URL.createObjectURL(img);
        image.src = url;
        imageSaveButton.href = url;
    }
});

type DiffOp = { "Replace": { new_index: number, new_len: number, old: string } }
    | { "Insert": { new_index: number, new_len: number } }
    | { "Delete": { new_index: number, old: string } };

const diffButton = document.getElementById("diff");
diffButton!.addEventListener("click", diff);

class LineBoxIter {
    #startIdx: number;
    #box: HTMLInputElement;
    constructor() {
        this.#startIdx = 0;
        if (boxContainer?.firstElementChild instanceof HTMLInputElement === false) throw new Error("where box?");
        this.#box = boxContainer.firstElementChild;
    }

    next(index: number, length: number) {
        while (index >= this.#box.value.length) {
            this.#startIdx += this.#box.value.length + 1;
            index -= this.#box.value.length + 1;
            if (this.#box.nextElementSibling instanceof HTMLInputElement === false) {
                debugger;
                throw new Error("where box?");
            }
            this.#box = this.#box.nextElementSibling;
        }
        this.#box.focus();
        this.#box.setSelectionRange(index, index + length);
    }
}

async function diff() {
    const body = Array.from(boxContainer!.children)
        .map((box) => box instanceof HTMLInputElement && box.value)
        .join(" ");
    const response = await fetch("/diff", {
        method: "POST",
        body,
    });
    if (response.status != 200) {
        throw new Error("Failed to diff");
    }
    const diff: DiffOp[] = await response.json();

    if (diff.length === 0) {
        if (diffButton instanceof HTMLInputElement === false) throw new Error("where diff button?");
        const icon = diffButton.value;
        diffButton.value = "✅";
        setTimeout(() => diffButton.value = icon, 2000);
    }

    const iter = new LineBoxIter();
    for (const op of diff) {
        if ("Replace" in op) {
            iter.next(op.Replace.new_index, op.Replace.new_len);
            return;
        } else if ("Insert" in op) {
            iter.next(op.Insert.new_index, op.Insert.new_len);
        } else if ("Delete" in op) {
            iter.next(op.Delete.new_index, 0);
        } else {
            throw new Error(`invalid op ${JSON.stringify(op)}`);
        }
        return;
    }
};

