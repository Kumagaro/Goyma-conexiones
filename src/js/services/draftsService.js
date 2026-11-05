import { invoke } from "./invoke.js";

export function saveDraft(formKey, recordId, payload) {
  return invoke("save_draft", { formKey, recordId, payload });
}

export function getDraft(formKey, recordId = null) {
  return invoke("get_draft", { formKey, recordId });
}

export function listDrafts() {
  return invoke("list_drafts");
}

export function deleteDraft(formKey, recordId = null) {
  return invoke("delete_draft", { formKey, recordId });
}
