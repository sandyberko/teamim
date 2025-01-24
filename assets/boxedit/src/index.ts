document.getElementById("input-fields")!.removeAttribute("disabled");

const imageStorageKey = "image";
const boxFileStorageKey = "boxfile";

let image: HTMLImageElement | null = null;

const main = document.getElementById("main");
if (main instanceof HTMLElement === false) throw new Error("where main?");

const boxContainer = document.getElementById("box-container");
if (boxContainer instanceof HTMLElement) { } else { throw new Error("where box container?"); }


// Image
function renderImage(imageData: string) {
    if (image === null) {
        image = new Image();
        document.getElementById("main")!.appendChild(image);
    }
    image.onload = (event) => {
        if (event.target instanceof HTMLImageElement === false) throw new Error("where image?");
        boxContainer!.style.width = event.target.width + 'px';
        boxContainer!.style.height = event.target.height + 'px';
    }

    image.src = imageData;
}

const imageInput = document.getElementById("image-input");
if (imageInput instanceof HTMLInputElement === false) throw new Error("where image input?");

imageInput.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    const reader = new FileReader();
    reader.onload = (event) => {
        const imageData = event.target?.result as string;
        localStorage.setItem(imageStorageKey, imageData);
        renderImage(imageData);
    }
    reader.readAsDataURL(input.files![0]);
});

document.getElementById("box-input")!.addEventListener("change", (event) => {
    const file = event.target as HTMLInputElement;
    const reader = new FileReader();
    reader.readAsText(file.files![0], 'UTF-8');
    reader.onload = function (event) {
        const text = event.target?.result as string;

        localStorage.setItem(boxFileStorageKey, text);

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
    boxElem.maxLength = 1;
    boxElem.minLength = 1;
    boxElem.style.left = left + 'px';
    boxElem.style.top = top + 'px';
    boxElem.style.width = width + 'px';
    boxElem.style.height = height + 'px';
    setupKeyboardResize(boxElem);
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
        if (event.data && event.data.match(hebrewLetterRegex) === null) event.preventDefault();
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

// Visibility
const viewAttr = "data-view";
const imageKey = "1";
const boxKey = "2";
const boxTextKey = "3";
const soloBoxKey = "4";

document.addEventListener("keydown", (event) => {
    switch (event.key) {
        case imageKey:
            if (!image) break;
            image.style.opacity = "0";
            break;
        case boxKey: boxContainer.style.opacity = "0"; break;
        case boxTextKey: boxContainer.setAttribute(viewAttr, "text-only"); break;
        case soloBoxKey: boxContainer.setAttribute(viewAttr, "solo"); break;
    }
});
document.addEventListener("keyup", (event) => {
    if (event.shiftKey) return;
    switch (event.key) {
        case imageKey:
            if (!image) break;
            image.style.opacity = "1";
            break;
        case boxKey: boxContainer.style.opacity = "1"; break;
        case boxTextKey:
        case soloBoxKey:
            boxContainer.removeAttribute(viewAttr);
            break;
    }
});

const saveButton = document.getElementById("save");
if (saveButton instanceof HTMLInputElement === false) throw new Error("where save button?");
let outputFile: FileSystemFileHandle | null = null;
saveButton.addEventListener("click", async (event) => {
    // create a new handle
    if (!outputFile) outputFile = await window.showSaveFilePicker();

    // create a FileSystemWritableFileStream to write to
    const writableStream = await outputFile.createWritable();

    // write our file
    for (const box of boxContainer.childNodes) {
        if (box instanceof HTMLInputElement === false) continue;

        const char = box.value;
        const left = box.offsetLeft;
        const bottom = box.offsetTop + box.offsetHeight;
        const right = box.offsetLeft + box.offsetWidth;
        const top = box.offsetTop;

        await writableStream.write(`${char} ${left} ${bottom} ${right} ${top} 0\n`);
    }

    // close the file and write the contents to disk.
    await writableStream.close();
});

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
    const text = await response.text();
    renderBoxes(text);
});

const storedImage = localStorage.getItem(imageStorageKey);
if (storedImage) {
    renderImage(storedImage);
}

const storedBoxFile = localStorage.getItem(boxFileStorageKey);
if (storedBoxFile) {
    renderBoxes(storedBoxFile);
}