"use strict";

import { TessBox, newBox } from "./tess-box.js";

let image: HTMLImageElement | null = null;

const main = document.getElementById("main") as HTMLElement;

window.addEventListener("keypress", (event) => {
  if (event.key === "`" || event.key === ";") {
    const legend = document.getElementById("legend");
    if (legend instanceof HTMLDialogElement === false)
      throw new Error("where legend?");
    legend.showModal();
  }
});

function boxContainer(): HTMLElement {
  const elem = document.getElementById("box-container");
  if (elem === null) {
    const container = document.createElement("div");
    container.id = "box-container";
    main.appendChild(container);
    return container;
  } else if (elem instanceof HTMLElement) {
    return elem;
  } else {
    throw new Error("where box container?");
  }
}

const imageInput = document.getElementById("image-input");
if (imageInput instanceof HTMLInputElement === false)
  throw new Error("where image input?");

const imageSaveButton = document.getElementById("image-save");
if (imageSaveButton instanceof HTMLAnchorElement === false)
  throw new Error("where save button?");

imageInput.addEventListener("change", (event) => {
  const input = event.target as HTMLInputElement;
  if (input.files === null || input.files.length === 0) return;
  const imageData = input.files[0];
  setImage(URL.createObjectURL(imageData));
});

const boxInput = document.getElementById("box-input") as HTMLInputElement;
boxInput.addEventListener("click", () => (boxInput.value = ""));
boxInput.addEventListener("change", (event) => {
  const file = event.target as HTMLInputElement;
  if (file.files === null || file.files.length === 0) return;
  const files = Array.from(file.files);

  if (targetInput.value === "training-preproc") {
    const html = files.find((file) => file.name.endsWith(".html"));
    if (!html) throw new Error("expected html");
    const reader = new FileReader();
    reader.onload = function (event) {
      const text = event.target?.result as string;
      boxContainer().outerHTML = text;
      if (image) {
        boxContainer().style.width = image.width + "px";
        boxContainer().style.height = image.height + "px";
      }
    };
    reader.readAsText(html, "UTF-8");
    return true;
  }

  const imageFile = files.find(
    (file) => file.name.endsWith(".jpg") || file.name.endsWith(".jpeg"),
  );
  if (imageFile) {
    setImage(URL.createObjectURL(imageFile));
  }
  const boxFile = files.find((file) => file.name.endsWith(".box"));
  if (boxFile) {
    const reader = new FileReader();
    reader.readAsText(boxFile, "UTF-8");
    reader.onload = function (event) {
      const text = event.target?.result as string;
      if (targetInput instanceof HTMLSelectElement === false)
        throw new Error("where target input?");
      switch (targetInput.value) {
        case "training":
          renderTrainingBoxes(text);
          break;
        case "recognition":
          renderBoxes(text, true);
          break;
      }
    };
  }
});

function setImage(src: string): HTMLImageElement {
  main.innerHTML = "";

  if (image === null) {
    image = new Image();
    main.appendChild(image);
  }

  image.src = src;

  new ResizeObserver((entries, observer) => {
    const image = entries[0].target;
    if (image instanceof HTMLImageElement === false)
      throw new Error("where image?");

    if (image.width === 0) return;
    
    setZoom((main.clientWidth * getZoom()) / image.width);

    observer.disconnect();
  }).observe(image);

  return image;
}

// #region Render boxes
function renderBoxes(text: string, training = false) {
  boxContainer().innerHTML = "";
  let lastBox: TessBox | null = null;
  for (const box of text.split("\n")) {
    if (box === "") continue;
    const char = box.substring(0, 1);
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
  }
}
// #endregion

// #region Training Boxes

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
      lastBox.prepend(document.createTextNode(char));
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
  }
}

// #endregion

{
  // Visibility
  const keyMap: Record<string, HTMLElement | null> = {
    // image
    "1": document.getElementById("view-image"),
    // box
    "2": document.getElementById("view-boxes"),
    // hide text
    "3": document.getElementById("view-text"),
    // solo box
    "4": document.getElementById("view-solo"),
  };
  document.addEventListener("keydown", (event) => {
    if (event.key in keyMap) {
      event.preventDefault();
      const elem = keyMap[event.key];
      if (elem instanceof HTMLInputElement === false)
        throw new Error(`expected input for ${event.key}`);
      elem.checked = false;
    }
  });
  document.addEventListener("keyup", (event) => {
    if (event.altKey) return;
    if (event.key in keyMap) {
      event.preventDefault();
      const elem = keyMap[event.key];
      if (elem instanceof HTMLInputElement === false)
        throw new Error(`expected input for ${event.key}`);
      elem.checked = true;
    }
  });
}

const targetInput = document.getElementById(
  "recognize-target",
) as HTMLInputElement;
targetInput.addEventListener("change", () => {
  switch (targetInput.value) {
    case "training":
      boxInput.accept = ".box,image/*";
      break;
    case "training-preproc":
      boxInput.accept = "text/html";
      break;
    case "recognition":
      boxInput.accept = ".box";
      break;
  }
});
let outputFile: FileSystemFileHandle | null = null;
{
  const boxSaveButton = document.getElementById("box-save") as HTMLInputElement;
  boxSaveButton.addEventListener("click", async () => {
    const icon = boxSaveButton.value;
    boxSaveButton.value = "⏳";
    boxSaveButton.disabled = true;

    try {
      switch (targetInput.value) {
        case "training": {
          const [origin, suggestedName]: [BoxFileOrigin, string | undefined] =
            boxContainer().hasAttribute("data-from-server")
              ? ["server", location.hash.substring(1).replaceAll("/", "_")]
              : boxInput.files !== null && boxInput.files.length > 0
                ? [
                    "file",
                    Array.from(boxInput.files).find((file) =>
                      file.name.endsWith(".box"),
                    )?.name,
                  ]
                : [
                    "server",
                    (($) => $ && $.substring(0, $.lastIndexOf(".")))(
                      imageInput.files?.[0]?.name,
                    ),
                  ];
          outputFile = await writeTrainingBoxes(
            outputFile,
            suggestedName,
            origin,
          );
          break;
        }
        case "training-preproc": {
          const fileName = location.hash.substring(1);
          if (fileName === "") throw new Error(`not a preprocessed file`);
          const res = await fetch(`/api/saveDiff?file=${fileName}`, {
            method: "POST",
            body: boxContainer().outerHTML,
          });

          if (res.status !== 200) {
            throw new Error("Failed to save preprocessed file");
          }
          break;
        }
        case "recognition": {
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

            await writableStream.write(
              `${char} ${left} ${bottom} ${right} ${top} 0\n`,
            );
          }

          // close the file and write the contents to disk.
          await writableStream.close();
          break;
        }
      }

      boxSaveButton.value = "✅";
    } catch (e) {
      boxSaveButton.value = "⚠️";
      throw e;
    } finally {
      setTimeout(() => {
        boxSaveButton.value = icon;
        boxSaveButton.disabled = false;
      }, 2000);
    }
  });
}

type BoxFileOrigin = "server" | "file";
async function writeTrainingBoxes(
  outputFile: FileSystemFileHandle | null,
  suggestedName: string | undefined,
  origin: BoxFileOrigin,
): Promise<FileSystemFileHandle> {
  if (!boxContainer) throw new Error("where boxContainer?");

  if (!outputFile)
    outputFile = await window.showSaveFilePicker({
      suggestedName,
      types: [{ description: "Box file", accept: { "text/plain": [".box"] } }],
    });

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
    const right =
      box.offsetLeft + box.offsetWidth + (origin == "server" ? 5 : 0);
    const top = imgHeight - box.offsetTop;

    // there seems to be a trailing space when coming from server
    for (const char of Array.from(box.innerText).reverse()) {
      await writableStream.write(
        `${char} ${left} ${bottom} ${right} ${top} 0\n`,
      );
    }
    await writableStream.write(`\t ${left} ${bottom} ${right} ${top} 0\n`);
  }

  // close the file and write the contents to disk.
  await writableStream.close();
  return outputFile;
}

function getBoxes() {
  if (boxContainer instanceof HTMLElement === false)
    throw new Error("where main?");
  return Array.from(boxContainer().childNodes)
    .map((box) => {
      if (box instanceof HTMLInputElement === false)
        throw new Error("invalid box");
      const char = box.value;
      const left = box.offsetLeft;
      const bottom = box.offsetTop + box.offsetHeight;
      const right = box.offsetLeft + box.offsetWidth;
      const top = box.offsetTop;
      return `${char} ${left} ${bottom} ${right} ${top} 0`;
    })
    .join("\n");
}

const recognizeButton = document.getElementById(
  "recognize",
) as HTMLInputElement;
recognizeButton.addEventListener("click", async (event) => {
  // get image from input
  if (imageInput.files === null || imageInput.files.length === 0) return;
  const imageData = imageInput.files[0];

  if (event.target instanceof HTMLInputElement === false)
    throw new Error("where recognize input?");
  event.target.disabled = true;
  const icon = event.target.value;
  event.target.value = "⏳";
  try {
    const response = await fetch(
      `/api/recognize${targetInput.value === "training" ? "Training" : ""}`,
      {
        method: "POST",
        body: imageData,
      },
    );
    if (response.status === 400) {
      imageInput.focus();
      alert("לא נבחרה תמונה");
    }
    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Failed to recognize: ${text}`);
    }
    switch (targetInput.value) {
      case "recognition": {
        const text = await response.text();
        renderBoxes(text, true);
        break;
      }
      case "training": {
        const boxes = await response.text();
        boxContainer().outerHTML = boxes;
        break;
      }
    }
  } finally {
    event.target.value = icon;
    event.target.disabled = false;
  }
});

const renderTeamimButton = document.getElementById(
  "render-teamim",
) as HTMLInputElement;
renderTeamimButton.addEventListener("click", async () => {
  const formData = new FormData();
  if (imageInput.files === null) throw new Error("where image?");
  formData.append("image", imageInput.files[0]);
  formData.append("boxes", getBoxes());
  const response = await fetch("/api/renderTeamim", {
    method: "POST",
    body: formData,
  });
  // parse error
  if (response.status === 400) {
    const error = await response.json();
    if (typeof error === "string" && error === "Not found") {
      alert("טקסט לא נמצא, נסה לתקן טעויות זיהוי ולהריץ שוב");
    } else {
      const { boxNumber } = error;
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

type DiffOp =
  | { Replace: { new_index: number; new_len: number; old: string } }
  | { Insert: { new_index: number; new_len: number } }
  | { Delete: { new_index: number; old: string } };

const diffButton = document.getElementById("diff");
if (diffButton instanceof HTMLInputElement === false)
  throw new Error("where diff button?");
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
  if (diffButton instanceof HTMLInputElement === false)
    throw new Error("where diff button?");
  const icon = diffButton.value;
  diffButton.value = "⏳";
  diffButton.disabled = true;
  try {
    const body = Array.from(boxContainer().children)
      .map((box) => box instanceof TessBox && box.innerText)
      .join(" ");
    const response = await fetch("/api/diff", {
      method: "POST",
      body,
    });
    if (response.status != 200) {
      throw new Error("Failed to diff");
    }
    const diff: DiffOp[] = await response.json();

    if (diff.length === 0) {
      diffButton.value = "✅";
      setTimeout(() => (diffButton.value = icon), 2000);
    } else {
      diffButton.value = "⚠️";
      setTimeout(() => (diffButton.value = icon), 2000);
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
}

const fontSizeInput = document.getElementById("font-size");
if (fontSizeInput instanceof HTMLInputElement === false)
  throw new Error("where font size input?");
fontSizeInput.addEventListener("input", () => {
  const fontSize = parseFloat(fontSizeInput.value);
  boxContainer().style.fontSize = fontSize + "em";
});

// #region Zoom
let _zoom: number;
const zoomInput = document.getElementById("zoom");
if (zoomInput instanceof HTMLInputElement === false)
  throw new Error("where zoom input?");
zoomInput.addEventListener(
  "input",
  (event) =>
    event.target instanceof HTMLInputElement &&
    setZoom(event.target.valueAsNumber),
);

export function setZoom(zoom: number) {
  _zoom = zoom;
  main.style.zoom = _zoom.toString();
  if (zoomInput instanceof HTMLInputElement) zoomInput.value = _zoom.toString();
}

setZoom(0.6);

export function getZoom(): number {
  return _zoom;
}

main.addEventListener("wheel", (event) => {
  if (event.ctrlKey === false) return;
  event.preventDefault();
  const zoom = getZoom();
  setZoom(zoom - event.deltaY / 100);
});
// #endregion

// NOTE: this should be last
// diffs
async function loadDiff(url: string) {
  main.innerHTML = "";
  image = null;
  outputFile = null;

  if (url === "") {
    setZoom(1);

    // index
    const distancesRes = await fetch("/diffs/distances.txt");
    if (distancesRes.status !== 200) {
      const errorElem = document.createElement("p");
      errorElem.classList.add("error");
      errorElem.innerText = "שגיאה בהורדת הרשימה";
      main.appendChild(errorElem);
      throw new Error("failed to load diffs/distances.txt");
    }
    const distances = await distancesRes.text();
    const table = document.createElement("table");
    table.id = "distances";
    const tbody = document.createElement("tbody");
    let i = 0;
    for (const line of distances.split("\n")) {
      const [distance, url] = line.split("\t");
      const tr = document.createElement("tr");
      tr.addEventListener("click", () => (location.hash = url));
      {
        const td = document.createElement("td");
        td.innerText = i.toString();
        tr.appendChild(td);
      }
      {
        const td = document.createElement("td");
        td.innerText = distance;
        tr.appendChild(td);
      }
      {
        const td = document.createElement("td");
        td.innerText = url;
        tr.appendChild(td);
      }
      tbody.appendChild(tr);
      i++;
    }
    table.appendChild(tbody);
    main.appendChild(table);
  } else {
    // image
    const image = setImage(`/images/${url}.jpg`);

    // boxes
    let response = await fetch(`/correctedDiffs/${url}.html`);
    if (response.status === 404) {
      response = await fetch(`/diffs/${url}.html`);
    }
    if (response.status !== 200) {
      throw new Error("Failed to load diffs");
    }
    const text = await response.text();
    boxContainer().outerHTML = text;
    boxContainer().toggleAttribute("data-from-server", true);
    if (image.complete) {
      boxContainer().style.width = image.width + "px";
      boxContainer().style.height = image.height + "px";
    } else {
      image.addEventListener(
        "load",
        () => {
          if (image === null) throw new Error("where image?");
          boxContainer().style.width = image.width + "px";
          boxContainer().style.height = image.height + "px";
        },
        { once: true },
      );
    }
  }
}
window.addEventListener("hashchange", () =>
  loadDiff(location.hash.substring(1)),
);
const diffUrl = location.hash.substring(1);
loadDiff(diffUrl);
