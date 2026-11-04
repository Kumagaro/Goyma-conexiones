import { invoke } from "./invoke.js";

// filters: { unreadOnly, search, date } (NT-01).
export function listNotifications(filters = {}) {
  return invoke("list_notifications", { filters });
}

export function countUnreadNotifications() {
  return invoke("count_unread_notifications");
}

export function listNotificationsSince(afterId) {
  return invoke("list_notifications_since", { afterId });
}

export function markNotificationRead(notificationId) {
  return invoke("mark_notification_read", { notificationId });
}

export function markAllNotificationsRead() {
  return invoke("mark_all_notifications_read");
}

export function deleteNotification(notificationId) {
  return invoke("delete_notification", { notificationId });
}

export function deleteNotifications(filters = {}) {
  return invoke("delete_notifications", { filters });
}
