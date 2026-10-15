const STAMP = /^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?/;
const UTC_STAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?Z$/;

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/*
 * Présente un horodatage au format français, à l'heure locale du poste.
 * La base enregistre en UTC (suffixe Z) ; les filtres de période et les
 * exports utilisent eux aussi la date locale, pour que tout concorde.
 */
export function formatDateTime(value: string | null | undefined, withSeconds = false): string {
  if (!value) {
    return "—";
  }
  if (UTC_STAMP.test(value)) {
    const date = new Date(value);
    if (!Number.isNaN(date.getTime())) {
      const day = `${pad(date.getDate())}/${pad(date.getMonth() + 1)}/${date.getFullYear()}`;
      const time = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
      return `${day} ${withSeconds ? `${time}:${pad(date.getSeconds())}` : time}`;
    }
  }
  const match = STAMP.exec(value);
  if (!match) {
    return value;
  }
  const [, year, month, day, hour, minute, second] = match;
  const time = withSeconds && second ? `${hour}:${minute}:${second}` : `${hour}:${minute}`;
  return `${day}/${month}/${year} ${time}`;
}

export function formatToday(now = new Date()): string {
  const text = now.toLocaleDateString("fr-FR", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

export const BACKUP_REMINDER_DAYS = 7;

/*
 * Rappel affiché sur le tableau de bord. La base n'existe que sur ce poste :
 * sans copie récente, une panne du disque fait perdre tout l'historique.
 */
export function backupReminder(
  createdAt: string | null | undefined,
  now = new Date(),
): { tone: "warn" | "danger"; text: string } | null {
  if (!createdAt) {
    return { tone: "danger", text: "Aucune sauvegarde n'a encore été réalisée sur ce poste." };
  }
  const saved = new Date(createdAt);
  if (Number.isNaN(saved.getTime())) {
    return null;
  }
  const days = Math.floor((now.getTime() - saved.getTime()) / 86_400_000);
  if (days < BACKUP_REMINDER_DAYS) {
    return null;
  }
  return {
    tone: "warn",
    text: `Dernière sauvegarde réussie il y a ${days} jours (${formatDateTime(createdAt)}).`,
  };
}
