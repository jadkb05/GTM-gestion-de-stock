import { Fragment, useEffect, useState, type FormEvent } from "react";
import { toErrorMessage } from "../app/api";
import type { NamedRecord } from "../catalogue/types";
import { validateLabel } from "../catalogue/validation";
import { plural } from "../app/format";
import { buttonClass, controlClass, Field, quietButtonClass } from "./Field";
import { StatusBadge } from "./StatusBadge";
import { Callout } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { EmptyState, Loading, PageHeader } from "./ui/Layout";

type ReferenceListProps = {
  title: string;
  addTitle?: string;
  note: string;
  emptyMessage: string;
  emptyNameMessage: string;
  load: () => Promise<NamedRecord[]>;
  create: (name: string) => Promise<NamedRecord>;
  update: (id: number, name: string) => Promise<NamedRecord>;
  setActive: (id: number, active: boolean) => Promise<NamedRecord>;
};

export function ReferenceList({
  title,
  addTitle = "Ajouter",
  note,
  emptyMessage,
  emptyNameMessage,
  load,
  create,
  update,
  setActive,
}: ReferenceListProps) {
  const [records, setRecords] = useState<NamedRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [nameError, setNameError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingName, setEditingName] = useState("");
  const [editingError, setEditingError] = useState<string | null>(null);
  const [pendingDisableId, setPendingDisableId] = useState<number | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      setRecords(await load());
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    load()
      .then((next) => {
        if (!cancelled) {
          setRecords(next);
          setLoading(false);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(toErrorMessage(reason));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [load]);

  async function submitCreate(event: FormEvent) {
    event.preventDefault();
    if (saving) {
      return;
    }
    const validation = validateLabel(name, emptyNameMessage);
    setNameError(validation);
    if (validation) {
      return;
    }
    setSaving(true);
    setNotice(null);
    try {
      await create(name);
      setName("");
      setNotice("Enregistrement effectué.");
      await refresh();
    } catch (reason) {
      setNameError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function submitEdit(id: number) {
    if (saving) {
      return;
    }
    const validation = validateLabel(editingName, emptyNameMessage);
    setEditingError(validation);
    if (validation) {
      return;
    }
    setSaving(true);
    setNotice(null);
    try {
      await update(id, editingName);
      setEditingId(null);
      setNotice("Modification enregistrée.");
      await refresh();
    } catch (reason) {
      setEditingError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function changeActive(record: NamedRecord, active: boolean) {
    setSaving(true);
    setNotice(null);
    try {
      await setActive(record.id, active);
      setPendingDisableId(null);
      setNotice(active ? "Entrée réactivée." : "Entrée désactivée. Les produits existants sont conservés.");
      await refresh();
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  const activeCount = records.filter((record) => record.isActive).length;

  return (
    <section>
      <PageHeader title={title} description={note} />

      {notice ? (
        <Callout tone="ok" role="status" className="mb-4">
          {notice}
        </Callout>
      ) : null}
      {error ? (
        <Callout
          tone="danger"
          role="alert"
          className="mb-4"
          action={
            <button type="button" className={`${quietButtonClass} btn-sm`} onClick={() => void refresh()}>
              Réessayer
            </button>
          }
        >
          {error}
        </Callout>
      ) : null}

      <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_19rem]">
        <div className="min-w-0">
          {loading && records.length === 0 ? <Loading>Chargement…</Loading> : null}
          {!loading && !error && records.length === 0 ? <EmptyState icon="tag" title={emptyMessage} /> : null}
          {records.length > 0 ? (
            <>
              <div className="table-frame table-frame-tall">
                <table className="data-table min-w-[30rem]">
                  <thead>
                    <tr>
                      <th>Nom</th>
                      <th>Statut</th>
                      <th className="actions">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {records.map((record) => (
                      <Fragment key={record.id}>
                        <tr className={record.isActive ? undefined : "row-muted"}>
                          <td className="w-full">
                            {editingId === record.id ? (
                              <div className="max-w-sm">
                                <label htmlFor={`edit-${record.id}`} className="sr-only">
                                  Nouveau nom
                                </label>
                                <input
                                  id={`edit-${record.id}`}
                                  value={editingName}
                                  aria-invalid={Boolean(editingError)}
                                  onChange={(event) => setEditingName(event.target.value)}
                                  onKeyDown={(event) => {
                                    if (event.key === "Enter") {
                                      event.preventDefault();
                                      void submitEdit(record.id);
                                    }
                                  }}
                                  className="control control-sm"
                                  autoFocus
                                />
                                {editingError ? (
                                  <p className="mt-1 text-meta font-medium text-danger">{editingError}</p>
                                ) : null}
                              </div>
                            ) : (
                              <span className={`font-medium ${record.isActive ? "text-ink" : "text-mist"}`}>
                                {record.name}
                              </span>
                            )}
                          </td>
                          <td>
                            <StatusBadge tone={record.isActive ? "ok" : "neutral"}>
                              {record.isActive ? "Actif" : "Désactivé"}
                            </StatusBadge>
                          </td>
                          <td className="actions">
                            <div className="flex items-center justify-end gap-1">
                              {editingId === record.id ? (
                                <>
                                  <button
                                    type="button"
                                    className="btn btn-primary btn-sm"
                                    disabled={saving}
                                    onClick={() => void submitEdit(record.id)}
                                  >
                                    Enregistrer
                                  </button>
                                  <button type="button" className="btn btn-secondary btn-sm" onClick={() => setEditingId(null)}>
                                    Annuler
                                  </button>
                                </>
                              ) : (
                                <button
                                  type="button"
                                  className="btn btn-ghost btn-sm"
                                  onClick={() => {
                                    setEditingId(record.id);
                                    setEditingName(record.name);
                                    setEditingError(null);
                                  }}
                                >
                                  Modifier
                                </button>
                              )}
                              {record.isActive ? (
                                pendingDisableId === record.id ? (
                                  <span className="inline-block min-w-[5.75rem]" aria-hidden="true" />
                                ) : (
                                  <button
                                    type="button"
                                    className="btn btn-ghost btn-sm min-w-[5.75rem] text-mist hover:text-danger"
                                    onClick={() => setPendingDisableId(record.id)}
                                  >
                                    Désactiver
                                  </button>
                                )
                              ) : (
                                <button
                                  type="button"
                                  className="btn btn-ghost btn-sm min-w-[5.75rem]"
                                  disabled={saving}
                                  onClick={() => void changeActive(record, true)}
                                >
                                  Réactiver
                                </button>
                              )}
                            </div>
                          </td>
                        </tr>
                        {record.isActive && pendingDisableId === record.id ? (
                          <tr className="row-warn">
                            <td colSpan={3} className="bg-warn-bg!">
                              <div className="flex flex-wrap items-center gap-3 py-1">
                                <p className="min-w-0 flex-1 text-table text-[#5f3a00]">
                                  Les produits déjà associés restent en place. Cette entrée ne pourra plus être
                                  choisie pour un nouveau produit.
                                </p>
                                <div className="flex gap-1.5">
                                  <button
                                    type="button"
                                    className="btn btn-danger btn-sm"
                                    disabled={saving}
                                    onClick={() => void changeActive(record, false)}
                                  >
                                    Confirmer la désactivation
                                  </button>
                                  <button
                                    type="button"
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => setPendingDisableId(null)}
                                  >
                                    Annuler
                                  </button>
                                </div>
                              </div>
                            </td>
                          </tr>
                        ) : null}
                      </Fragment>
                    ))}
                  </tbody>
                </table>
              </div>
              <p className="mt-2 text-meta text-faint">
                {plural(records.length, "entrée", "entrées")} · {plural(activeCount, "active", "actives")}
              </p>
            </>
          ) : null}
        </div>

        <form onSubmit={submitCreate} className="panel" noValidate>
          <div className="panel-header">
            <h2 className="panel-title">{addTitle}</h2>
          </div>
          <div className="space-y-3 p-4">
            <Field id="entry-name" label="Nom" error={nameError ?? undefined}>
              <input
                id="entry-name"
                value={name}
                aria-invalid={Boolean(nameError)}
                aria-describedby={nameError ? "entry-name-error" : undefined}
                onChange={(event) => setName(event.target.value)}
                className={controlClass}
              />
            </Field>
            <button type="submit" disabled={saving} className={`${buttonClass} w-full`}>
              <Icon name="plus" size={16} />
              Ajouter
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}
