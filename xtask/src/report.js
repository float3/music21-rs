// The reports page is static; this only adds what a static page cannot do —
// filtering the ported list and saying which section the reader is in. The
// page is complete without it, so everything here degrades to nothing.
(function () {
    document.documentElement.classList.remove("no-js");

    const list = document.querySelector("[data-class-list]");
    const search = document.querySelector("[data-filter-search]");
    const incomplete = document.querySelector("[data-filter-incomplete]");
    const tally = document.querySelector("[data-filter-tally]");
    const expand = document.querySelector("[data-expand-all]");
    const collapse = document.querySelector("[data-collapse-all]");

    if (list) {
        const items = Array.from(list.querySelectorAll(".class-item"));
        const empty = list.querySelector("[data-filter-empty]");

        const apply = () => {
            const needle = (search ? search.value : "").trim().toLowerCase();
            const onlyIncomplete = incomplete ? incomplete.checked : false;
            let shown = 0;
            for (const item of items) {
                const hit =
                    (!needle || item.dataset.search.includes(needle)) &&
                    (!onlyIncomplete || item.dataset.missing !== "0");
                item.hidden = !hit;
                if (hit) shown += 1;
            }
            if (empty) empty.hidden = shown !== 0;
            if (tally) {
                tally.textContent =
                    shown === items.length
                        ? items.length + " classes"
                        : shown + " of " + items.length + " classes";
            }
        };

        if (search) search.addEventListener("input", apply);
        if (incomplete) incomplete.addEventListener("change", apply);
        if (expand) {
            expand.addEventListener("click", () => {
                for (const item of items) if (!item.hidden) item.open = true;
            });
        }
        if (collapse) {
            collapse.addEventListener("click", () => {
                for (const item of items) item.open = false;
            });
        }
        apply();
    }

    const links = Array.from(document.querySelectorAll(".section-nav a"));
    const sections = links
        .map((link) => document.querySelector(link.getAttribute("href")))
        .filter(Boolean);
    if (sections.length && "IntersectionObserver" in window) {
        const seen = new Set();
        const observer = new IntersectionObserver(
            (entries) => {
                for (const entry of entries) {
                    if (entry.isIntersecting) seen.add(entry.target);
                    else seen.delete(entry.target);
                }
                const current = sections.find((section) => seen.has(section));
                for (const link of links) {
                    link.classList.toggle(
                        "is-current",
                        !!current && link.getAttribute("href") === "#" + current.id,
                    );
                }
            },
            { rootMargin: "-80px 0px -55% 0px" },
        );
        for (const section of sections) observer.observe(section);
    }
})();
