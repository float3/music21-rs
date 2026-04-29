const STYLE_ID = "music21-help-tooltips-style";
const TOOLTIP_ID = "music21-help-tooltip";
const EDGE_GAP = 12;
const TRIGGER_GAP = 8;

function ensureTooltipStyles(): void {
  if (document.getElementById(STYLE_ID)) return;

  const style = document.createElement("style");
  style.id = STYLE_ID;
  style.textContent = `
    .help::after {
      content: none !important;
      display: none !important;
    }

    .floating-help-tooltip {
      position: fixed;
      z-index: 10000;
      width: max-content;
      max-width: min(340px, calc(100vw - 24px));
      padding: 9px 10px;
      border: 1px solid var(--line, rgba(255, 255, 255, 0.16));
      border-radius: 8px;
      background: var(--tooltip-bg, #171717);
      color: var(--tooltip-ink, #ffffff);
      box-shadow: 0 10px 24px rgba(0, 0, 0, 0.2);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      font-size: 12px;
      font-weight: 600;
      line-height: 1.35;
      opacity: 0;
      pointer-events: none;
      text-align: left;
      text-transform: none;
      transform: translateY(-2px);
      transition:
        opacity 120ms ease,
        transform 120ms ease;
      white-space: normal;
    }

    .floating-help-tooltip.visible {
      opacity: 1;
      transform: translateY(0);
    }
  `;
  document.head.appendChild(style);
}

function ensureTooltip(): HTMLDivElement {
  let tooltip = document.getElementById(TOOLTIP_ID) as HTMLDivElement | null;
  if (tooltip) return tooltip;

  tooltip = document.createElement("div");
  tooltip.id = TOOLTIP_ID;
  tooltip.className = "floating-help-tooltip";
  tooltip.setAttribute("role", "tooltip");
  tooltip.hidden = true;
  document.body.appendChild(tooltip);
  return tooltip;
}

function helpTriggerFromEvent(event: Event): HTMLElement | null {
  if (!(event.target instanceof Element)) return null;
  return event.target.closest<HTMLElement>(".help[data-help]");
}

export function setupHelpTooltips(): void {
  ensureTooltipStyles();
  const tooltip = ensureTooltip();
  let activeTrigger: HTMLElement | null = null;

  function positionTooltip(): void {
    if (!activeTrigger || tooltip.hidden) return;

    const triggerRect = activeTrigger.getBoundingClientRect();
    const tooltipRect = tooltip.getBoundingClientRect();
    const maxLeft = Math.max(
      EDGE_GAP,
      window.innerWidth - tooltipRect.width - EDGE_GAP,
    );
    const maxTop = Math.max(
      EDGE_GAP,
      window.innerHeight - tooltipRect.height - EDGE_GAP,
    );
    const centeredLeft =
      triggerRect.left + triggerRect.width / 2 - tooltipRect.width / 2;
    let left = Math.min(Math.max(centeredLeft, EDGE_GAP), maxLeft);
    let top = triggerRect.bottom + TRIGGER_GAP;

    if (top + tooltipRect.height + EDGE_GAP > window.innerHeight) {
      top = triggerRect.top - tooltipRect.height - TRIGGER_GAP;
    }

    left = Math.max(EDGE_GAP, left);
    top = Math.min(Math.max(top, EDGE_GAP), maxTop);

    tooltip.style.left = `${left}px`;
    tooltip.style.top = `${top}px`;
  }

  function showTooltip(trigger: HTMLElement): void {
    const text = trigger.dataset.help;
    if (!text) return;

    if (trigger.hasAttribute("title")) {
      trigger.dataset.originalTitle = trigger.getAttribute("title") ?? "";
      trigger.removeAttribute("title");
    }

    activeTrigger = trigger;
    tooltip.textContent = text;
    tooltip.hidden = false;
    tooltip.classList.add("visible");
    trigger.setAttribute("aria-describedby", TOOLTIP_ID);
    positionTooltip();
  }

  function hideTooltip(trigger?: HTMLElement | null): void {
    if (trigger && activeTrigger !== trigger) return;

    activeTrigger?.removeAttribute("aria-describedby");
    activeTrigger = null;
    tooltip.classList.remove("visible");
    tooltip.hidden = true;
  }

  document.addEventListener("pointerover", (event) => {
    const trigger = helpTriggerFromEvent(event);
    if (trigger) showTooltip(trigger);
  });

  document.addEventListener("pointerout", (event) => {
    const trigger = helpTriggerFromEvent(event);
    if (!trigger) return;
    if (
      event instanceof PointerEvent &&
      event.relatedTarget instanceof Node &&
      trigger.contains(event.relatedTarget)
    ) {
      return;
    }
    hideTooltip(trigger);
  });

  document.addEventListener("focusin", (event) => {
    const trigger = helpTriggerFromEvent(event);
    if (trigger) showTooltip(trigger);
  });

  document.addEventListener("focusout", (event) => {
    const trigger = helpTriggerFromEvent(event);
    if (trigger) hideTooltip(trigger);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") hideTooltip();
  });

  window.addEventListener("scroll", positionTooltip, true);
  window.addEventListener("resize", positionTooltip);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", setupHelpTooltips, { once: true });
} else {
  setupHelpTooltips();
}
