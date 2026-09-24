/// The small DOM helpers every page builds its panels with.

/// An element with an optional class and text.
export function el<K extends keyof HTMLElementTagNameMap>(
    tag: K,
    className?: string,
    text?: string,
): HTMLElementTagNameMap[K] {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
}

/// A labelled value, as the fact panels show one.
export function fact(label: string, value: string): HTMLDivElement {
    const node = el("div", "fact");
    node.append(el("span", "", label), el("strong", "", value));
    return node;
}

/// Shows a message in a page's error line, and hides the line again.
export function errorLine(node: HTMLElement): { fail(message: string): void; clearError(): void } {
    return {
        fail(message: string): void {
            node.textContent = message;
            node.style.display = "block";
        },
        clearError(): void {
            node.style.display = "none";
        },
    };
}

/// Copies text to the clipboard, through a hidden text area where the
/// clipboard API is not available (an insecure context, an older browser).
export async function writeClipboard(value: string): Promise<void> {
    if (navigator.clipboard?.writeText && window.isSecureContext) {
        await navigator.clipboard.writeText(value);
        return;
    }

    const textarea = document.createElement("textarea");
    textarea.value = value;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.top = "-1000px";
    document.body.appendChild(textarea);
    textarea.select();
    try {
        if (!document.execCommand("copy")) {
            throw new Error("Copy command failed");
        }
    } finally {
        textarea.remove();
    }
}
