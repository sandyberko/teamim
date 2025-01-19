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
    Inside = 0b1111,
}
// TODO use bitflags
function getDir(top: boolean, right: boolean, bottom: boolean, left: boolean): Direction {
    if (top && !right && !bottom && !left) { return Direction.Top; }
    if (top && right && !bottom && !left) { return Direction.TopRight; }
    if (!top && right && !bottom && !left) { return Direction.Right; }
    if (!top && right && bottom && !left) { return Direction.RightBottom; }
    if (!top && !right && bottom && !left) { return Direction.Bottom; }
    if (!top && !right && bottom && left) { return Direction.BottomLeft; }
    if (!top && !right && !bottom && left) { return Direction.Left; }
    if (top && !right && !bottom && left) { return Direction.LeftTop; }
    if (!top && !right && !bottom && !left) { return Direction.Inside; }

    throw new Error(`invalid direction ${[top, right, bottom, left]}`);
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

const handleMouseOver = (event: MouseEvent) => {
    if (event.target instanceof HTMLInputElement === false) return false;
    event.target.style.cursor = cursor.get(eventDir(event)) || (() => { throw new Error("invalid cursor") })();
}
let prev: { elem: HTMLInputElement, clientX: number, clientY: number, dir: Direction } | null = null;
function handleMouseDown(event: MouseEvent) {
    if (event.target instanceof HTMLInputElement === false) return false;
    prev = {
        elem: event.target,
        dir: eventDir(event),
        clientX: event.clientX,
        clientY: event.clientY,
    };
    boxContainer!.addEventListener("mousemove", handleMouseMove);
    boxContainer!.addEventListener("mouseup", handleMouseUp);
}
function handleMouseMove(event: MouseEvent) {
    if (prev === null) return false;
    const dx = event.clientX - prev.clientX, dy = event.clientY - prev.clientY;
    prev.clientX = event.clientX;
    prev.clientY = event.clientY;

    if ((prev.dir & Direction.Top) != 0) {
        prev.elem.style.top = prev.elem.offsetTop + dy + 'px';
        prev.elem.style.height = prev.elem.clientHeight - dy + 'px';
    }
    if ((prev.dir & Direction.Right) != 0) {
        prev.elem.style.width = prev.elem.clientWidth + dx + 'px';
    }
    if ((prev.dir & Direction.Bottom) != 0) {
        prev.elem.style.height = prev.elem.clientHeight + dy + 'px';
    }
    if ((prev.dir & Direction.Left) != 0) {
        prev.elem.style.left = prev.elem.offsetLeft + dx + 'px';
        prev.elem.style.width = prev.elem.clientWidth - dx + 'px';
    }
}
function handleMouseUp(event: MouseEvent) {
    prev = null;
    if (event.target instanceof HTMLInputElement === false) return false;
    boxContainer!.removeEventListener("mousemove", handleMouseMove);
    boxContainer!.removeEventListener("mouseup", handleMouseUp);
}
boxContainer.addEventListener("focusin", (event) => {
    if (event.target instanceof HTMLInputElement === false) return false;
    event.target.style.zIndex = "2";
    event.target.addEventListener("mousedown", handleMouseDown);
    event.target.addEventListener("mouseover", handleMouseOver);
});
boxContainer.addEventListener("focusout", (event) => {
    if (event.target instanceof HTMLInputElement === false) return false;
    event.target.style.zIndex = "0";
    event.target.addEventListener("mousedown", handleMouseDown);
    event.target.removeEventListener("mouseover", handleMouseOver);
});
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