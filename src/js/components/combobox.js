export function createCombobox({
  container,
  id,
  placeholder = "",
  search,
  formatOption = (item) => String(item?.name ?? ""),
  addOptionLabel,
  onAddNew,
  onSelect,
  clearLabel = "",
  debounceMs = 300,
}) {
  if (!container || typeof search !== "function") {
    throw new TypeError("El combobox necesita un contenedor y una búsqueda.");
  }

  const input = document.createElement("input");
  input.type = "text";
  input.id = `${id}-input`;
  input.placeholder = placeholder;
  input.autocomplete = "off";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.setAttribute("aria-controls", `${id}-listbox`);

  const listbox = document.createElement("div");
  listbox.id = `${id}-listbox`;
  listbox.setAttribute("role", "listbox");
  listbox.hidden = true;

  const clearButton = document.createElement("button");
  clearButton.type = "button";
  clearButton.className = "btn-secondary";
  clearButton.textContent = clearLabel;
  clearButton.hidden = true;
  clearButton.dataset.comboboxClear = "";
  clearButton.setAttribute("aria-label", clearLabel || "Limpiar selección");

  const controlRow = document.createElement("div");
  controlRow.className = "autocomplete-row";
  controlRow.append(input, clearButton);
  container.replaceChildren(controlRow, listbox);

  let selectedItem = null;
  let options = [];
  let activeIndex = -1;
  let requestId = 0;
  let timer = null;
  let editing = false;

  function closeListbox() {
    listbox.hidden = true;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
    activeIndex = -1;
  }

  function updateClearButton() {
    clearButton.hidden = !(editing && selectedItem && clearLabel);
  }

  function setSelected(item) {
    selectedItem = item ?? null;
    input.value = selectedItem ? formatOption(selectedItem) : "";
    closeListbox();
    updateClearButton();
    onSelect?.(selectedItem);
  }

  function clearSelection() {
    setSelected(null);
    input.focus();
  }

  function setActiveOption(nextIndex) {
    if (!options.length) return;
    activeIndex = (nextIndex + options.length) % options.length;
    options.forEach((option, index) => {
      option.element.setAttribute(
        "aria-selected",
        String(index === activeIndex),
      );
    });
    input.setAttribute(
      "aria-activedescendant",
      options[activeIndex].element.id,
    );
  }

  function chooseOption(option) {
    if (option.kind === "add") {
      if (typeof onAddNew !== "function") return;
      const query = input.value.trim();
      Promise.resolve(onAddNew(query))
        .then((item) => {
          if (item) setSelected(item);
        })
        .catch(() => {});
      closeListbox();
      return;
    }
    setSelected(option.item);
  }

  function addOption(item, index, kind = "match") {
    const option = document.createElement("button");
    option.type = "button";
    option.id = `${id}-option-${index}`;
    option.className = "action-link";
    option.setAttribute("role", "option");
    option.setAttribute("aria-selected", "false");
    if (kind === "add") option.dataset.comboboxAdd = "";
    option.textContent =
      kind === "add" ? addOptionLabel(input.value.trim()) : formatOption(item);
    option.addEventListener("mousedown", (event) => event.preventDefault());
    option.addEventListener("click", () => chooseOption({ kind, item }));
    listbox.append(option);
    return { element: option, item, kind };
  }

  async function searchAndRender(query, currentRequestId) {
    try {
      const matches = await search(query);
      if (currentRequestId !== requestId) return;
      const items = Array.isArray(matches) ? matches : [];
      listbox.replaceChildren();
      activeIndex = -1;
      input.removeAttribute("aria-activedescendant");
      options = items.map((item, index) => addOption(item, index));

      const exactMatch = items.some((item) => item?.isExactMatch === true);
      if (query && !exactMatch && typeof onAddNew === "function") {
        options.push(addOption(null, options.length, "add"));
      }

      if (options.length) {
        listbox.hidden = false;
        input.setAttribute("aria-expanded", "true");
      } else {
        closeListbox();
      }
    } catch {
      if (currentRequestId === requestId) {
        listbox.replaceChildren();
        options = [];
        closeListbox();
      }
    }
  }

  input.addEventListener("input", () => {
    selectedItem = null;
    updateClearButton();
    onSelect?.(null);
    const query = input.value.trim();
    const currentRequestId = ++requestId;
    if (timer !== null) clearTimeout(timer);
    if (!query) {
      listbox.replaceChildren();
      options = [];
      closeListbox();
      return;
    }
    timer = setTimeout(
      () => {
        void searchAndRender(query, currentRequestId);
      },
      Math.max(0, debounceMs),
    );
  });

  input.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" && options.length) {
      event.preventDefault();
      setActiveOption(activeIndex + 1);
    } else if (event.key === "ArrowUp" && options.length) {
      event.preventDefault();
      setActiveOption(activeIndex < 0 ? options.length - 1 : activeIndex - 1);
    } else if (event.key === "Enter" && activeIndex >= 0) {
      event.preventDefault();
      chooseOption(options[activeIndex]);
    } else if (event.key === "Escape") {
      closeListbox();
    } else if (event.key === "Tab") {
      closeListbox();
    }
  });

  clearButton.addEventListener("click", clearSelection);
  container.addEventListener("focusout", (event) => {
    if (!container.contains(event.relatedTarget)) {
      setTimeout(() => closeListbox(), 0);
    }
  });

  return {
    getInput: () => input,
    getSelected: () => selectedItem,
    getInputValue: () => input.value.trim(),
    hasUnselectedValue: () =>
      input.value.trim().length > 0 && selectedItem === null,
    setSelected,
    setEditing(value) {
      editing = Boolean(value);
      updateClearButton();
    },
    clear: clearSelection,
    destroy() {
      if (timer !== null) clearTimeout(timer);
      container.replaceChildren();
    },
  };
}
