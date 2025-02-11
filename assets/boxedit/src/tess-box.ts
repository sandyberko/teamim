export const TAG_NAME = 'tess-box';
export class TessBox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        const left = this.getAttribute("left");
        const bottom = this.getAttribute("bottom");
        const right = this.getAttribute("right");
        const top = this.getAttribute("top");

        if (left === null || bottom === null || right === null || top === null) throw new Error("missing box attribute");

        this.style.left = left + 'px';
        this.style.height = parseInt(bottom) - parseInt(top) + 'px';
        this.style.width = parseInt(right) - parseInt(left) + 'px';
        this.style.top = top + 'px';
    }
}
customElements.define(TAG_NAME, TessBox);