import { navGroups, sectionById, type SectionId } from "../app/navigation";
import logo from "../assets/gtm-batiment-logo.png";
import { Icon } from "./ui/Icon";

type SidebarProps = {
  current: SectionId;
  /* Module d'origine mis en évidence lorsqu'une fiche est ouverte. */
  context?: SectionId;
  onSelect: (section: SectionId) => void;
  alertCount: number | null;
};

export function Sidebar({ current, context, onSelect, alertCount }: SidebarProps) {
  return (
    <nav
      aria-label="Modules"
      className="flex h-full w-sidebar shrink-0 flex-col overflow-y-auto bg-graphite text-graphite-text"
    >
      <div className="bg-surface px-5 pb-3 pt-3.5">
        <img src={logo} alt="GTM Bâtiment" width={332} height={124} className="block h-auto w-[132px]" draggable={false} />
      </div>
      <div className="nav-product flex items-center justify-between border-b border-graphite-line px-5 py-2.5">
        <span className="text-[0.6875rem] font-semibold uppercase tracking-[0.12em] text-graphite-text">
          Stock Manager
        </span>
        <span className="h-0.5 w-5 bg-gtm-red" aria-hidden="true" />
      </div>
      <div className="flex-1 py-1.5">
        {navGroups.map((group) => (
          <div key={group.label} className="nav-group mt-1.5 first:mt-0">
            <p className="nav-group-label px-5 pb-1 pt-1.5 text-[0.65625rem] font-semibold uppercase tracking-[0.1em] text-graphite-muted">
              {group.label}
            </p>
            <ul className="space-y-px px-2.5">
              {group.items.map((id) => {
                const section = sectionById(id);
                const selected = id === current;
                const highlighted = selected || id === context;
                return (
                  <li key={id}>
                    <button
                      type="button"
                      aria-current={selected ? "page" : undefined}
                      onClick={() => onSelect(id)}
                      className={`nav-item relative flex h-[2.125rem] w-full items-center gap-2.5 rounded-control px-2.5 text-left text-[0.84375rem] transition-colors focus-visible:outline-2 focus-visible:outline-offset-0 focus-visible:outline-white/70 ${
                        highlighted
                          ? "bg-graphite-active font-medium text-white"
                          : "text-graphite-text hover:bg-graphite-raised hover:text-white"
                      }`}
                    >
                      {highlighted ? (
                        <span className="absolute inset-y-2 left-0 w-[3px] rounded-r bg-gtm-red" aria-hidden="true" />
                      ) : null}
                      <Icon name={section.icon} size={16} className={highlighted ? "text-white" : "text-graphite-muted"} />
                      <span className="min-w-0 flex-1 truncate">{section.label}</span>
                      {id === "alerts" && alertCount !== null ? (
                        <span
                          className={`min-w-6 rounded-full px-1.5 py-0.5 text-center text-[0.6875rem] font-semibold leading-none tabular-nums ${
                            alertCount > 0 ? "bg-gtm-red-strong text-white" : "bg-graphite-line text-graphite-muted"
                          }`}
                        >
                          {alertCount}
                        </span>
                      ) : null}
                      {section.ready ? null : <span className="text-meta text-graphite-muted">Indisponible</span>}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-2 border-t border-graphite-line px-5 py-3 text-meta text-graphite-muted">
        <Icon name="database" size={14} />
        Données locales de ce poste
      </div>
    </nav>
  );
}
