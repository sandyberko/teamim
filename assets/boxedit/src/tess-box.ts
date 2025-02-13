export const TAG_NAME = 'tess-box';
export class TessBox extends HTMLElement {

    handleDoubleClick(event: MouseEvent): boolean {
        if (event.target instanceof Element === false || event.target.tagName !== "INSERT") return false;
        const err = event.target.getAttribute("err");
        if (err === null) return false;
        event.target.replaceWith(err);
        return true;
    }

    connectedCallback() {
        this.addEventListener("dblclick", this.handleDoubleClick.bind(this));
        this.tabIndex = 0;

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

    constructor() {
        super();
    }
}
customElements.define(TAG_NAME, TessBox);