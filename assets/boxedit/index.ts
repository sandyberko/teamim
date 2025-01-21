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

const storedImage = localStorage.getItem(imageStorageKey);
if (storedImage) {
    renderImage(storedImage);
}

document.getElementById("image-input")!.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    const reader = new FileReader();
    reader.onload = (event) => {
        const imageData = event.target?.result as string;
        localStorage.setItem(imageStorageKey, imageData);
        renderImage(imageData);
    }
    reader.readAsDataURL(input.files![0]);
});

// #region Render boxes
function renderBoxes(text: string) {
    const boxContainer = document.getElementById("box-container");
    if (boxContainer instanceof HTMLElement === false) throw new Error("where main?");
    for (const line of text.split("\n")) {
        const box = line.trim();
        if (box === "") continue;
        const [char, left, top, right, bottom] = box.split(" ");
        const char_input = document.createElement("input");
        char_input.classList.add("box");
        char_input.type = "text";
        char_input.value = char;
        char_input.style.left = left + 'px';
        char_input.style.top = top + 'px';
        char_input.style.width = (parseInt(right) - parseInt(left)) + 'px';
        char_input.style.height = (parseInt(bottom) - parseInt(top)) + 'px';
        boxContainer.appendChild(char_input);
    };
}
const storedBoxFile = localStorage.getItem(boxFileStorageKey);
if (storedBoxFile) {
    renderBoxes(storedBoxFile);
}
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
// #endregion

// #region Resize
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
    const dir = eventDir(downEvent) || 0b1111;

    const controller = new AbortController();
    const signal = controller.signal;

    let prevX = downEvent.clientX, prevY = downEvent.clientY;
    boxContainer!.addEventListener("mousemove", (moveEvent) => {
        const dx = moveEvent.clientX - prevX, dy = moveEvent.clientY - prevY;
        prevX = moveEvent.clientX;
        prevY = moveEvent.clientY;

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
    }, { signal });

    boxContainer!.addEventListener("mouseup", () => controller.abort(), { signal });
}

{
    document.addEventListener("focusin", (event) => {
        if (event.target instanceof HTMLInputElement === false) return false;
        event.target.style.zIndex = "2";
        event.target.addEventListener("mousemove", handleMouseMove);
        event.target.addEventListener("mousedown", handleMouseDown);
    });
    document.addEventListener("focusout", (event) => {
        if (event.target instanceof HTMLInputElement === false) return false;
        event.target.style.zIndex = "0";
        event.target.removeEventListener("mousemove", handleMouseMove);
        event.target.removeEventListener("mousedown", handleMouseDown);
    });
}
// #endregion

// Visibility
const imageKey = "1";
const boxKey = "2";
const boxTextKey = "3";

document.addEventListener("keydown", (event) => {
    switch (event.key) {
        case imageKey:
            if (!image) break;
            image.style.visibility = "hidden";
            break;
        case boxKey: boxContainer.style.visibility = "hidden"; break;
        case boxTextKey: boxContainer.setAttribute("data-hide-text", "hide"); break;
    }
});
document.addEventListener("keyup", (event) => {
    if (event.shiftKey) return;
    switch (event.key) {
        case imageKey:
            if (!image) break;
            image.style.visibility = "visible";
            break;
        case boxKey: boxContainer.style.visibility = "visible"; break;
        case boxTextKey: boxContainer.removeAttribute("data-hide-text"); break;
    }
});

const saveButton = document.getElementById("save");
if (saveButton instanceof HTMLInputElement === false) throw new Error("where download button?");
saveButton.addEventListener("click", async (event) => {
    // create a new handle
    const newHandle = await window.showSaveFilePicker();

    // create a FileSystemWritableFileStream to write to
    const writableStream = await newHandle.createWritable();

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