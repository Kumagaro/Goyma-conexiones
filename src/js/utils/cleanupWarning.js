// Aviso discreto de purga fallida (plan-03 §5.12): se muestra con textContent
// en un elemento role="status" y se limpia cuando el backend manda null.
// Si la vista no tiene el elemento en el HTML, se crea al inicio del contenedor.
export function showCleanupWarning(container, id, warning) {
  if (!container) return;
  let status = document.getElementById(id);
  if (!status) {
    status = document.createElement("p");
    status.id = id;
    status.className = "form-feedback";
    status.setAttribute("role", "status");
    container.prepend(status);
  }
  status.textContent = warning ?? "";
  status.hidden = !warning;
}
