let nextDialogId = 0;

export function confirmDialog({
  title = "Confirmar acción",
  message = "",
  confirmText = "Confirmar",
  cancelText = "Cancelar",
} = {}) {
  const opener = document.activeElement;
  const overlay = document.createElement("div");
  overlay.className = "modal-overlay active";

  const card = document.createElement("div");
  card.className = "modal-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("tabindex", "-1");

  const titleId = `confirm-dialog-title-${++nextDialogId}`;
  card.setAttribute("aria-labelledby", titleId);

  const header = document.createElement("div");
  header.className = "modal-header";
  const heading = document.createElement("h3");
  heading.id = titleId;
  heading.textContent = title;
  header.append(heading);

  const messageContent = document.createElement("div");
  messageContent.className = "confirm-dialog-message";
  for (const line of String(message).split(/\r\n|\r|\n/)) {
    const paragraph = document.createElement("p");
    paragraph.textContent = line;
    messageContent.append(paragraph);
  }

  const actions = document.createElement("div");
  actions.className = "form-actions";
  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "btn-secondary";
  cancelButton.dataset.action = "cancel";
  cancelButton.textContent = cancelText;

  const confirmButton = document.createElement("button");
  confirmButton.type = "button";
  confirmButton.className = "btn-danger";
  confirmButton.dataset.action = "confirm";
  confirmButton.textContent = confirmText;
  actions.append(cancelButton, confirmButton);

  card.append(header, messageContent, actions);
  overlay.append(card);

  return new Promise((resolve) => {
    let settled = false;

    const finish = (confirmed) => {
      if (settled) return;
      settled = true;
      document.removeEventListener("keydown", handleKeydown);
      overlay.remove();
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
      resolve(confirmed);
    };

    const handleKeydown = (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        finish(false);
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = [...card.querySelectorAll("button:not(:disabled)")];
      const currentIndex = focusable.indexOf(document.activeElement);
      const direction = event.shiftKey ? -1 : 1;
      const nextIndex =
        currentIndex === -1
          ? 0
          : (currentIndex + direction + focusable.length) % focusable.length;
      event.preventDefault();
      focusable[nextIndex].focus();
    };

    cancelButton.addEventListener("click", () => finish(false));
    confirmButton.addEventListener("click", () => finish(true));
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) finish(false);
    });
    document.addEventListener("keydown", handleKeydown);
    document.body.append(overlay);
    cancelButton.focus();
  });
}
