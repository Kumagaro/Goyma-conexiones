// Atajos de teclado: Ctrl+S guarda el formulario a la vista y Ctrl+K lleva al
// buscador de la pantalla.

const isVisible = (node) => node?.offsetParent !== null;

function activeForm() {
  const dialogForm = document.querySelector(".modal-overlay.active form");
  if (dialogForm) return dialogForm;
  return [...document.querySelectorAll(".page.active form")].find(isVisible);
}

function activeSearch() {
  return [
    ...document.querySelectorAll(
      '.page.active .search-bar input, .page.active input[id$="-search"]',
    ),
  ].find(isVisible);
}

export function installShortcuts() {
  document.addEventListener("keydown", (event) => {
    if (!event.ctrlKey && !event.metaKey) return;
    const key = event.key.toLowerCase();
    if (key === "s") {
      event.preventDefault();
      activeForm()?.requestSubmit();
    } else if (key === "k") {
      event.preventDefault();
      activeSearch()?.focus();
    }
  });
}
