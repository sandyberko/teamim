"use strict";

import { TessBox, TAG_NAME, newBox } from "./tess-box.js";

let image: HTMLImageElement | null = null;

const main = document.getElementById("main");
if (main instanceof HTMLElement === false) throw new Error("where main?");

function boxContainer(): HTMLElement {
    const elem = document.getElementById("box-container");
    if (elem instanceof HTMLElement) {
        return elem;
    } else {
        throw new Error("where box container?");
    }

}

const imageInput = document.getElementById("image-input");
if (imageInput instanceof HTMLInputElement === false) throw new Error("where image input?");

const imageSaveButton = document.getElementById("save-image");
if (imageSaveButton instanceof HTMLAnchorElement === false) throw new Error("where save button?");


imageInput.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    const imageData = input.files![0];
    setImage(imageData);
});

const boxInput = document.getElementById("box-input");
if (boxInput instanceof HTMLInputElement === false) throw new Error("where box input?");
boxInput.addEventListener("change", (event) => {
    const file = event.target as HTMLInputElement;
    const reader = new FileReader();
    reader.readAsText(file.files![0], 'UTF-8');
    reader.onload = function (event) {
        const text = event.target?.result as string;
        renderBoxes(text);
    }
});

function setImage(imageData: File) {
    const url = URL.createObjectURL(imageData);
    if (image === null) {
        image = new Image();
        document.getElementById("main")!.appendChild(image);
    }
    image.onload = (event) => {
        if (event.target instanceof HTMLImageElement === false) throw new Error("where image?");
        boxContainer().style.width = event.target.width + 'px';
        boxContainer().style.height = event.target.height + 'px';
    };

    image.src = url;
}

// #region Render boxes
function renderBoxes(text: string, training: boolean = false) {
    boxContainer().innerHTML = "";
    let lastBox: TessBox | null = null;
    for (const box of text.split("\n")) {
        if (box === "") continue;
        let char = box.substring(0, 1);
        if (char === "\t") {
            lastBox = null;
        } else if (lastBox) {
            lastBox.appendChild(document.createTextNode(char));
        } else {
            const [left, bottom, right, top] = box.substring(2).split(" ");
            const width = parseInt(right) - parseInt(left);
            const height = parseInt(bottom) - parseInt(top);
            const boxElem = newBox(char, left, top, width, height);
            if (training) {
                lastBox = boxElem;
            }
            boxContainer().appendChild(boxElem);
        }
    };
}
// #endregion

// #region Training Boxes
{
    // lstm box format
    function renderTrainingBoxes(text: string) {
        if (!image) throw new Error("where image?");
        const imgHeight = image.height;

        boxContainer().innerHTML = "";
        let lastBox: TessBox | null = null;
        for (const line of text.split("\n")) {
            if (line === "") continue;
            const char = line.substring(0, 1);
            if (char === "\t") {
                lastBox = null;
            } else if (lastBox) {
                lastBox.appendChild(document.createTextNode(char));
                continue;
            } else {
                const [left, blBottom, right, blTop] = line.substring(2).split(" ");
                const top = imgHeight - parseInt(blTop);
                const bottom = imgHeight - parseInt(blBottom);
                const width = parseInt(right) - parseInt(left);
                const height = bottom - top;
                lastBox = newBox(char, left, top.toString(), width, height);
                boxContainer().appendChild(lastBox);
            }
        };
    }
    const trainingInput = document.getElementById("training-input") as HTMLInputElement;
    trainingInput!.addEventListener("change", (event) => {
        const file = event.target as HTMLInputElement;
        const files = Array.from(file.files!);
        let imageFile = files.find((file) => file.name.endsWith(".jpg") || file.name.endsWith(".jpeg"));
        if (imageFile) {
            setImage(imageFile);
        }
        const boxFile = files.find((file) => file.name.endsWith(".box"));
        if (boxFile) {
            const reader = new FileReader();
            reader.readAsText(boxFile, 'UTF-8');
            reader.onload = function (event) {
                const text = event.target?.result as string;
                renderTrainingBoxes(text);
            }
        }
    });

    let outputFile: FileSystemFileHandle | null = null;
    document.getElementById("training-save")!.addEventListener("click", async () => {
        // create a new handle
        const suggestedName = Array.from(trainingInput.files!).find((file) => file.name.endsWith(".box"))?.name;
        outputFile = await writeTrainingBoxes(outputFile, suggestedName);
    });
}
// #endregion

// Visibility
const viewAttr = "data-view";
const keyMap: Record<string, (active: boolean) => void> = {
    // image
    "1": (show) => { image && (image.style.opacity = show ? "1" : "0"); },
    // box
    "2": (show) => { boxContainer().style.opacity = show ? "1" : "0"; },
    // box + text
    "3": (show) => { show ? boxContainer().removeAttribute(viewAttr) : boxContainer().setAttribute(viewAttr, "text-only"); },
    // solo box
    "4": (show) => { show ? boxContainer().removeAttribute(viewAttr) : boxContainer().setAttribute(viewAttr, "solo"); },
    // diff
    "F1": (show) => { show && diff(); }
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

const targetInput = document.getElementById("recognize-target");
if (targetInput instanceof HTMLSelectElement === false) throw new Error("where target input?");
{
    let outputFile: FileSystemFileHandle | null = null;
    document.getElementById("save-box")!.addEventListener("click", async () => {
        switch (targetInput.value) {
            case "training":
                const [fromServer, suggestedName] = boxInput.files?.[0]
                    ? [false, boxInput.files[0].name]
                    : [true, (($) => $ && $.substring(0, $.lastIndexOf('.')))(imageInput.files?.[0]?.name)]
                outputFile = await writeTrainingBoxes(outputFile, suggestedName, fromServer);
                break;
            case "recognition":
                // create a new handle
                if (!outputFile) outputFile = await window.showSaveFilePicker();

                // create a FileSystemWritableFileStream to write to
                const writableStream = await outputFile.createWritable();
                // write our file
                for (const box of boxContainer().childNodes) {
                    if (box instanceof TessBox === false) continue;

                    // top-left custom box format
                    const char = box.innerText;
                    const left = box.offsetLeft;
                    const bottom = box.offsetTop + box.offsetHeight;
                    const right = box.offsetLeft + box.offsetWidth;
                    const top = box.offsetTop;

                    await writableStream.write(`${char} ${left} ${bottom} ${right} ${top} 0\n`);
                }

                // close the file and write the contents to disk.
                await writableStream.close();
                break;
        }
    });
}

async function writeTrainingBoxes(outputFile: FileSystemFileHandle | null, suggestedName: string | undefined, fromServer: boolean = false): Promise<FileSystemFileHandle> {
    if (!boxContainer) throw new Error("where boxContainer?");

    if (!outputFile) outputFile = await window.showSaveFilePicker({ suggestedName, types: [{ description: "Box file", accept: { "text/plain": [".box"] } }] });

    // create a FileSystemWritableFileStream to write to
    const writableStream = await outputFile.createWritable();

    if (!image) throw new Error("where image?");
    const imgHeight = image.height;

    // write our file
    for (const box of boxContainer().childNodes) {
        if (box instanceof TessBox === false) continue;

        // lstm box format
        const left = box.offsetLeft;
        const bottom = imgHeight - box.offsetTop - box.offsetHeight;
        // see [https://github.com/tesseract-ocr/tesseract/blob/3157ff0e741ea5c85e16fbd1c6edf20f30eccbd3/src/api/lstmboxrenderer.cpp#L34]
        const right = box.offsetLeft + box.offsetWidth + (fromServer ? 5 : 0);
        const top = imgHeight - box.offsetTop;

        // there seems to be a trailing space when coming from server
        for (const char of Array.from(box.innerText).reverse()) {
            await writableStream.write(`${char} ${left} ${bottom} ${right} ${top} 0\n`);
        }
        await writableStream.write(`\t ${left} ${bottom} ${right} ${top} 0\n`);
    }

    // close the file and write the contents to disk.
    await writableStream.close();
    return outputFile;
}

function getBoxes() {
    if (boxContainer instanceof HTMLElement === false) throw new Error("where main?");
    return Array.from(boxContainer().childNodes)
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

document.getElementById("recognize")!.addEventListener("click", async (event) => {
    // get image from input
    const imageData = imageInput.files![0];

    if (event.target instanceof HTMLInputElement === false) throw new Error("where recognize input?");
    event.target.disabled = true;
    const icon = event.target.value;
    event.target.value = "⏳";
    try {
        const response = await fetch(`/recognize${targetInput.value === "training" ? "Training" : ""}`, {
            method: "POST",
            body: imageData
        });
        if (response.status === 400) {
            imageInput.focus();
            alert("לא נבחרה תמונה");
        }
        if (!response.ok) {
            const text = await response.text();
            throw new Error(`Failed to recognize: ${text}`);
        }
        switch (targetInput.value) {
            case "recognition":
                const text = await response.text();
                renderBoxes(text, true);
                break;
            case "training":
                const boxes = await response.text();
                boxContainer().outerHTML = boxes;
                break;
        }
    }
    finally {
        event.target.value = icon;
        event.target.disabled = false;
    }
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
            const box = boxContainer().childNodes[boxNumber];
            if (box instanceof TessBox === false) throw new Error("where box?");
            box.focus();
            // box.select();
            // blink box
            box.animate([{ backgroundColor: "red" }, { backgroundColor: "white" }], {
                duration: 1000,
                iterations: 5,
            });
            // box.setCustomValidity(expected);
            // box.reportValidity();
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
if (diffButton instanceof HTMLInputElement === false) throw new Error("where diff button?");
diffButton.addEventListener("click", diff);

// class LineBoxIter {
//     #startIdx: number;
//     #box: TessBox;
//     constructor() {
//         this.#startIdx = 0;
//         if (boxContainer?.firstElementChild instanceof TessBox === false) throw new Error("where box?");
//         this.#box = boxContainer().firstElementChild;
//     }

//     next(index: number, length: number) {
//         while (index >= this.#box.value.length) {
//             this.#startIdx += this.#box.value.length + 1;
//             index -= this.#box.value.length + 1;
//             if (this.#box.nextElementSibling instanceof TessBox === false) {
//                 debugger;
//                 throw new Error("where box?");
//             }
//             this.#box = this.#box.nextElementSibling;
//         }
//         this.#box.focus();
//         this.#box.setSelectionRange(index, index + length);
//     }
// }

async function diff() {
    if (diffButton instanceof HTMLInputElement === false) throw new Error("where diff button?");
    const icon = diffButton.value;
    diffButton.value = "⏳";
    diffButton.disabled = true;
    try {
        const body = Array.from(boxContainer().children)
            .map((box) => box instanceof TessBox && box.innerText)
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
            diffButton.value = "✅";
            setTimeout(() => diffButton.value = icon, 2000);
        } else {
            diffButton.value = "⚠️";
            setTimeout(() => diffButton.value = icon, 2000);
            // const iter = new LineBoxIter();
            // for (const op of diff) {
            //     if ("Replace" in op) {
            //         iter.next(op.Replace.new_index, op.Replace.new_len);
            //         return;
            //     } else if ("Insert" in op) {
            //         iter.next(op.Insert.new_index, op.Insert.new_len);
            //     } else if ("Delete" in op) {
            //         iter.next(op.Delete.new_index, 0);
            //     } else {
            //         throw new Error(`invalid op ${JSON.stringify(op)}`);
            //     }
            //     break;
            // }
        }
    } finally {
        diffButton.disabled = false;
    }
};

