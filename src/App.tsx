import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";
import { revealItemInDir, openPath } from "@tauri-apps/plugin-opener";

interface ArchiveObject {
  id: string;
  display_name: string;
  status: string;
  description: string | null;
  file_path: string;
  file_size_bytes: number;
  checksum_sha256: string;
  storage_type: string;   // 'managed' | 'reference'
  is_accessible: boolean; // для reference: чи існує файл за шляхом
  created_at: string;
}

interface TagInfoDTO { id: string; tag_name: string; created_at: string; usage_count: number; }
interface PersonInfoDTO { id: string; person_name: string; created_at: string; usage_count: number; }
interface LocationInfoDTO { id: string; location_name: string; created_at: string; usage_count: number; }
interface ExportInfoDTO { name: string; path: string; size_bytes: number; modified_at: string; }

interface AuditLogDTO {
  id: string;
  action_type: string;
  target_record_id: string | null;
  details: string | null;
  created_at: string;
}
interface IngestResult { object_id: string; duplicate_detected: boolean; }

interface FixityProblem {
  object_id: string;
  display_name: string;
  file_path: string;
  status: string; // 'mismatch' | 'missing' | 'unreadable'
  expected_sha256: string;
  actual_sha256: string;
}
interface FixityReport {
  checked: number;
  ok: number;
  problems: FixityProblem[];
}
interface DocumentCard {
  id: string;
  display_name: string;
  file_kind: string | null;
  author: string | null;
  purpose: string | null;
  doc_date_from: string | null;
  doc_date_to: string | null;
  source_location: string | null;
  description: string | null;
}

type TabType = "archive" | "tags" | "persons" | "locations" | "exports" | "audit";

// Іконка типу файлу за розширенням
function fileIcon(path: string): string {
  const ext = (path.split(".").pop() || "").toLowerCase();
  if (["jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff", "heic"].includes(ext)) return "🖼️";
  if (ext === "pdf") return "📕";
  if (["doc", "docx", "rtf", "odt", "pages"].includes(ext)) return "📄";
  if (["xls", "xlsx", "csv", "numbers", "ods"].includes(ext)) return "📊";
  if (["ppt", "pptx", "key", "odp"].includes(ext)) return "📽️";
  if (["mp3", "wav", "aac", "flac", "m4a"].includes(ext)) return "🎵";
  if (["mp4", "mov", "avi", "mkv"].includes(ext)) return "🎬";
  if (["txt", "md", "rtfd"].includes(ext)) return "📝";
  if (["zip", "rar", "7z", "tar", "gz"].includes(ext)) return "🗜️";
  if (["eml", "msg"].includes(ext)) return "✉️";
  return "📁";
}

export default function App() {
  const [activeTab, setActiveTab] = useState<TabType>("archive");

  const [objects, setObjects] = useState<ArchiveObject[]>([]);
  const [selectedObject, setSelectedObject] = useState<ArchiveObject | null>(null);

  // Довільний опис поточного вибраного документа
  const [description, setDescription] = useState<string>("");

  const [tags, setTags] = useState<string[]>([]);
  const [newTag, setNewTag] = useState<string>("");
  const [persons, setPersons] = useState<string[]>([]);
  const [newPerson, setNewPerson] = useState<string>("");
  const [locations, setLocations] = useState<string[]>([]);
  const [newLocation, setNewLocation] = useState<string>("");

  const [allTags, setAllTags] = useState<TagInfoDTO[]>([]);
  const [allPersons, setAllPersons] = useState<PersonInfoDTO[]>([]);
  const [allLocations, setAllLocations] = useState<LocationInfoDTO[]>([]);
  const [auditLogs, setAuditLogs] = useState<AuditLogDTO[]>([]);

  const [exportFiles, setExportFiles] = useState<ExportInfoDTO[]>([]);
  const [renamingExport, setRenamingExport] = useState<string | null>(null); // path файла, який перейменовуємо
  const [renameValue, setRenameValue] = useState<string>("");

  // Фільтри реєстру
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [filterTagId, setFilterTagId] = useState<string>("");
  const [filterPersonId, setFilterPersonId] = useState<string>("");
  const [filterLocationId, setFilterLocationId] = useState<string>("");
  const [filterExt, setFilterExt] = useState<string>("");
  const [filterDateFrom, setFilterDateFrom] = useState<string>("");
  const [filterDateTo, setFilterDateTo] = useState<string>("");
  const [filterMinKb, setFilterMinKb] = useState<string>("");
  const [filterMaxKb, setFilterMaxKb] = useState<string>("");
  const [filterDocDateFrom, setFilterDocDateFrom] = useState<string>("");
  const [filterDocDateTo, setFilterDocDateTo] = useState<string>("");
  const [page, setPage] = useState<number>(0);
  const [totalObjects, setTotalObjects] = useState<number>(0);
  const [fixityReport, setFixityReport] = useState<FixityReport | null>(null);
 
  // Режим перегляду реєстру + кеш мініатюр (object_id → data-URL або null)
  const [viewMode, setViewMode] = useState<"table" | "gallery">("table");
  const [thumbs, setThumbs] = useState<Record<string, string | null>>({});
  // Архівна картка документа (редагована частина)
  const [card, setCard] = useState<DocumentCard | null>(null);

  // CRUD стани
  const [editingTagId, setEditingTagId] = useState<string | null>(null);
  const [editingTagName, setEditingTagName] = useState<string>("");
  const [mergeSourceId, setMergeSourceId] = useState<string>("");
  const [mergeTargetId, setMergeTargetId] = useState<string>("");

  const [editingPersonId, setEditingPersonId] = useState<string | null>(null);
  const [editingPersonName, setEditingPersonName] = useState<string>("");
  const [mergePersonSource, setMergePersonSource] = useState<string>("");
  const [mergePersonTarget, setMergePersonTarget] = useState<string>("");

  const [editingLocationId, setEditingLocationId] = useState<string | null>(null);
  const [editingLocationName, setEditingLocationName] = useState<string>("");
  const [mergeLocationSource, setMergeLocationSource] = useState<string>("");
  const [mergeLocationTarget, setMergeLocationTarget] = useState<string>("");

  const [status, setStatus] = useState<string>("");
  const [loading, setLoading] = useState<boolean>(false);
  // Режим додавання: 'managed' — копія в архів, 'reference' — лише посилання
  const [ingestMode, setIngestMode] = useState<"managed" | "reference">("managed");
  // Чи включати підпапки при імпорті каталогу
  const [recursiveIngest, setRecursiveIngest] = useState<boolean>(false);

  const PAGE_SIZE = 50;

  // Єдине місце, де збираються всі фільтри (для списку і для підрахунку)
  const buildFilters = () => ({
    query: searchQuery || null,
    tag_id: filterTagId || null,
    person_id: filterPersonId || null,
    location_id: filterLocationId || null,
    extension: filterExt.trim() || null,
    date_from: filterDateFrom || null,
    date_to: filterDateTo || null,
    doc_date_from: filterDocDateFrom || null,
    doc_date_to: filterDocDateTo || null,
    min_size: Number.isFinite(Number(filterMinKb)) && filterMinKb.trim() !== ""
      ? Math.round(Number(filterMinKb) * 1024) : null,
    max_size: Number.isFinite(Number(filterMaxKb)) && filterMaxKb.trim() !== ""
      ? Math.round(Number(filterMaxKb) * 1024) : null,
    limit: PAGE_SIZE,
    offset: page * PAGE_SIZE,
  });

  const fetchFilteredObjects = async () => {
    try {
      const filters = buildFilters();
      const res = await invoke<ArchiveObject[]>("search_objects_v2", { filters });
      setObjects(res);
      if (viewMode === "gallery") fetchThumbnails(res);
      // Загальна кількість збігів (без пагінації) — щоб порахувати сторінки
      const total = await invoke<number>("count_objects_command", {
        filters: { ...filters, limit: null, offset: null },
      });
      setTotalObjects(total);
      // Оновлюємо вибраний об'єкт, якщо він змінився
      if (selectedObject) {
        const updated = res.find(o => o.id === selectedObject.id);
        if (updated) setSelectedObject(updated);
      }
    } catch (err) {
      console.error("Помилка пошуку:", err);
    }
  };

  const totalPages = Math.max(1, Math.ceil(totalObjects / PAGE_SIZE));

  const fetchMetadata = async (objectId: string) => {
    try {
      setTags(await invoke<string[]>("get_object_tags", { objectId }));
      setPersons(await invoke<string[]>("get_object_persons", { objectId }));
      setLocations(await invoke<string[]>("get_object_locations", { objectId }));
    } catch (err) { console.error(err); }
  };

  // Завантажує мініатюри лише для тих документів, яких ще немає в кеші
    const fetchThumbnails = async (objs: ArchiveObject[], force = false) => {
    const needed = force ? objs : objs.filter(o => !(o.id in thumbs));
    if (needed.length === 0) return;
    const results = await Promise.all(needed.map(async o => {
      const t = await invoke<string | null>("get_thumbnail_command", { objectId: o.id }).catch(() => null);
      return [o.id, t] as const;
    }));
    setThumbs(prev => {
      const next = { ...prev };
      for (const [id, t] of results) next[id] = t;
      return next;
    });
  };

    const fetchCard = async (objectId: string) => {
    try {
      setCard(await invoke<DocumentCard>("get_document_card_command", { objectId }));
    } catch (err) { console.error("Помилка картки:", err); }
  };

  const setCardField = (field: keyof DocumentCard, value: string) =>
    setCard(prev => (prev ? { ...prev, [field]: value } : prev));

  const fetchAllData = async () => {
    try {
      setAllTags(await invoke<TagInfoDTO[]>("get_all_tags_modal"));
      setAllPersons(await invoke<PersonInfoDTO[]>("get_all_persons_modal"));
      setAllLocations(await invoke<LocationInfoDTO[]>("get_all_locations_modal"));
      setAuditLogs(await invoke<AuditLogDTO[]>("get_audit_logs_modal"));
    } catch (err) { console.error(err); }
  };

  useEffect(() => {
    fetchFilteredObjects();
    fetchAllData();
  }, [searchQuery, filterTagId, filterPersonId, filterLocationId,
      filterExt, filterDateFrom, filterDateTo, filterMinKb, filterMaxKb, page, filterDocDateFrom, filterDocDateTo]);
 
  // При переході в галерею — підтягуємо мініатюри для поточної сторінки
  useEffect(() => {
    if (viewMode === "gallery") fetchThumbnails(objects);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewMode]);

  // Зміна будь-якого фільтра скидає реєстр на першу сторінку
  useEffect(() => {
    setPage(0);
  }, [searchQuery, filterTagId, filterPersonId, filterLocationId, filterExt, filterDateFrom, filterDateTo, filterMinKb, filterMaxKb, filterDocDateFrom, filterDocDateTo]);

  const handleRowClick = (obj: ArchiveObject) => {
    setSelectedObject(obj);
    setDescription(obj.description || "");
    fetchCard(obj.id);
    fetchMetadata(obj.id);
  };

  const fetchExports = async () => {
    try {
      setExportFiles(await invoke<ExportInfoDTO[]>("list_exports_command"));
    } catch (err) { console.error(err); }
  };

  // Експорт: тепер файл пише Rust у exports/, фронтенд лише повідомляє шлях
  const handleExportCsv = async () => {
    try {
      const path = await invoke<string>("save_export_command", { kind: "csv" });
      setStatus(`✅ Експорт CSV збережено: ${path}`);
      fetchExports();
    } catch (err: any) { setStatus(`Помилка експорту CSV: ${String(err)}`); }
  };

  const handleExportJson = async () => {
    try {
      const path = await invoke<string>("save_export_command", { kind: "json" });
      setStatus(`✅ Експорт JSON збережено: ${path}`);
      fetchExports();
    } catch (err: any) { setStatus(`Помилка експорту JSON: ${String(err)}`); }
  };

  const handleRenameExport = async (oldName: string) => {
    try {
      await invoke("rename_export_command", { oldName, newName: renameValue });
      setRenamingExport(null); setRenameValue("");
      fetchExports();
      setStatus("Файл перейменовано.");
    } catch (err: any) { setStatus(`Помилка перейменування: ${String(err)}`); }
  };

  // Відкрити Finder і підсвітити файл
  const handleRevealExport = async (path: string) => {
    try { await revealItemInDir(path); }
    catch (err: any) { setStatus(`Не вдалося відкрити теку: ${String(err)}`); }
  };

  // Відкрити сам файл у програмі за замовчуванням
  const handleOpenExport = async (path: string) => {
    try { await openPath(path); }
    catch (err: any) { setStatus(`Не вдалося відкрити файл: ${String(err)}`); }
  };

  // Збереження довільного опису документа
  const handleSaveDescription = async () => {
    if (!selectedObject) return;
    try {
      await invoke("update_object_description_handler", { objectId: selectedObject.id, description });
      alert("Опис успішно збережено!");
      fetchFilteredObjects();
      fetchAllData();
    } catch (err) {
      console.error("Помилка збереження опису:", err);
      alert(`Помилка: ${err}`);
    }
  };

    // Збереження архівної картки (тип, автор, призначення, дати, розташування)
  const handleSaveCard = async () => {
    if (!selectedObject || !card) return;
    try {
      await invoke("update_document_card_command", {
        card: {
          object_id: selectedObject.id,
          file_kind: card.file_kind || null,
          author: card.author || null,
          purpose: card.purpose || null,
          doc_date_from: card.doc_date_from || null,
          doc_date_to: card.doc_date_to || null,
          source_location: card.source_location || null,
        },
      });
      setStatus("✅ Картку документа оновлено.");
      fetchFilteredObjects();
      fetchAllData();
      fetchCard(selectedObject.id);
    } catch (err: any) {
      setStatus(`Помилка збереження картки: ${String(err)}`);
    }
  };

  const handleAddTag = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedObject || !newTag.trim()) return;
    await invoke("add_tag_to_object", { objectId: selectedObject.id, tagName: newTag.trim() });
    setNewTag("");
    fetchMetadata(selectedObject.id);
    fetchAllData();
    fetchFilteredObjects();
  };

  const handleAddPerson = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedObject || !newPerson.trim()) return;
    await invoke("add_person_to_object", { objectId: selectedObject.id, tagName: newPerson.trim() });
    setNewPerson("");
    fetchMetadata(selectedObject.id);
    fetchAllData();
    fetchFilteredObjects();
  };

  const handleAddLocation = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedObject || !newLocation.trim()) return;
    await invoke("add_location_to_object", { objectId: selectedObject.id, tagName: newLocation.trim() });
    setNewLocation("");
    fetchMetadata(selectedObject.id);
    fetchAllData();
    fetchFilteredObjects();
  };

  const handleMergeTags = async () => {
    if (!mergeSourceId || !mergeTargetId) return alert("Оберіть вихідний та цільовий теги!");
    if (!confirm("Виконати злиття тегів?")) return;
    await invoke("merge_tags_handler", { sourceTagId: mergeSourceId, targetTagId: mergeTargetId });
    setMergeSourceId(""); setMergeTargetId("");
    fetchAllData(); fetchFilteredObjects(); if (selectedObject) fetchMetadata(selectedObject.id);
  };

  const handleMergePersons = async () => {
    if (!mergePersonSource || !mergePersonTarget) return alert("Оберіть вихідну та цільову особу!");
    if (!confirm("Виконати злиття персоналій?")) return;
    await invoke("merge_persons_handler", { sourceId: mergePersonSource, targetId: mergePersonTarget });
    setMergePersonSource(""); setMergePersonTarget("");
    fetchAllData(); fetchFilteredObjects(); if (selectedObject) fetchMetadata(selectedObject.id);
  };

  const handleMergeLocations = async () => {
    if (!mergeLocationSource || !mergeLocationTarget) return alert("Оберіть вихідну та цільову локацію!");
    if (!confirm("Виконати злиття локацій?")) return;
    await invoke("merge_locations_handler", { sourceId: mergeLocationSource, targetId: mergeLocationTarget });
    setMergeLocationSource(""); setMergeLocationTarget("");
    fetchAllData(); fetchFilteredObjects(); if (selectedObject) fetchMetadata(selectedObject.id);
  };

  const handleSelectAndIngest = async () => {
    try {
      const selected = await open({ multiple: false, directory: false });
      if (!selected) return;
      const filePath = Array.isArray(selected) ? selected[0] : selected;
      if (!filePath) return;

      setLoading(true);
      if (ingestMode === "reference") {
        setStatus("Індексація зовнішнього файлу (без копіювання)...");
        await invoke("index_reference_file", { sourcePath: filePath });
        setStatus("Посилання збережено в каталозі. Оригінал залишається на своєму місці.");
      } else {
        setStatus("Імпортування файлу...");
        const result = await invoke<IngestResult>("ingest_file", { sourcePath: filePath });
        setStatus(result.duplicate_detected
          ? "Файл збережено. Увага: у каталозі вже є копія з таким SHA-256."
          : "Файл успішно збережено в архів!");
      }
      fetchFilteredObjects();
      fetchAllData();
    } catch (err: any) {
      setStatus(`Помилка: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  // Імпорт каталогу з вибором: з підпапками чи без
  const handleSelectDirectory = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return;
      const dirPath = Array.isArray(selected) ? selected[0] : selected;
      if (!dirPath) return;

      setLoading(true);
      const modeText = recursiveIngest ? " (включно з підпапками)" : "";
      setStatus(`Імпортування каталогу${modeText}... Це може зайняти деякий час.`);

      const count = await invoke<number>("ingest_directory_command", {
        dirPath,
        recursive: recursiveIngest,
      });

      setStatus(`Каталог імпортовано! Додано ${count} файлів.`);
      fetchFilteredObjects();
      fetchAllData();
    } catch (err: any) {
      setStatus(`Помилка імпорту каталогу: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  // Перевірка доступності всіх зовнішніх (reference) посилань
  const handleReconcile = async () => {
    try {
      const changed = await invoke<number>("reconcile_reference_locations");
      setStatus(changed === 0
        ? "Перевірку посилань виконано: всі зовнішні файли доступні."
        : `Перевірку посилань виконано: оновлено статус для ${changed} записів.`);
      fetchFilteredObjects();
      fetchAllData();
    } catch (err: any) {
      setStatus(`Помилка перевірки посилань: ${String(err)}`);
    }
  };

  // Перевірка цілісності ВСЬОГО архіву: перерахунок SHA-256 кожного managed-файлу
  const handleFullFixity = async () => {
    try {
      setLoading(true);
      setFixityReport(null);
      setStatus("🩺 Перевірка цілісності всього архіву... Це може зайняти деякий час.");
      const report = await invoke<FixityReport>("verify_archive_fixity_command");
      setFixityReport(report);
      setStatus(`🩺 Перевірено ${report.checked} файлів: цілих ${report.ok}, проблем ${report.problems.length}.`);
      fetchAllData();
    } catch (err: any) {
      setStatus(`Помилка перевірки архіву: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  // Резервне копіювання: обираємо папку, КУДИ покласти копію
  // (усередині буде створена dps-backup-<дата-час>/ з базою, storage/ та manifest.json)
  const handleCreateBackup = async () => {
    try {
      const dir = await open({ directory: true, multiple: false });
      if (!dir) return;
      setLoading(true);
      setStatus("Створення резервної копії...");
      const createdPath = await invoke<string>("create_backup_command", {
        backupDir: dir as string,
      });
      setStatus(`✅ Копію створено: ${createdPath}`);
    } catch (err: any) {
      setStatus(`Помилка копіювання: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  // Відновлення: обираємо САМУ папку копії dps-backup-... (де лежить manifest.json)
  const handleRestoreBackup = async () => {
    try {
      const dir = await open({ directory: true, multiple: false });
      if (!dir) return;
      if (!confirm("Відновити архів із цієї копії? Поточна база буде замінена даними з копії.")) return;
      setLoading(true);
      setStatus("Відновлення архіву...");
      await invoke("restore_backup_command", { backupPath: dir as string });
      setSelectedObject(null);
      await fetchFilteredObjects();
      fetchAllData();
      setStatus("✅ Архів відновлено з копії.");
    } catch (err: any) {
      setStatus(`Помилка відновлення: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  return (
        <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          Chartularius
          <small>цифровий архів</small>
        </div>
      </header>

      {/* Верхні вкладки */}
      <nav className="tabs">
        <button onClick={() => setActiveTab("archive")} className={activeTab === "archive" ? "on" : ""}>🗂 Архівовані файли</button>
        <button onClick={() => { setActiveTab("tags"); fetchAllData(); }} className={activeTab === "tags" ? "on" : ""}>🏷 Теги</button>
        <button onClick={() => { setActiveTab("persons"); fetchAllData(); }} className={activeTab === "persons" ? "on" : ""}>👤 Люди</button>
        <button onClick={() => { setActiveTab("locations"); fetchAllData(); }} className={activeTab === "locations" ? "on" : ""}>📍 Місця</button>
        <button onClick={() => { setActiveTab("exports"); fetchExports(); }} className={activeTab === "exports" ? "on" : ""}>📤 Експорти</button>
        <button onClick={() => { setActiveTab("audit"); fetchAllData(); }} className={activeTab === "audit" ? "on" : ""}>📜 Журнал дій</button>
      </nav>

      {activeTab === "archive" && (
        <>
          {/* ПАНЕЛЬ ДІЙ НАД АРХІВОМ */}
          <div className="actions">
            <div className="seg">
              <button onClick={() => setIngestMode("managed")} className={ingestMode === "managed" ? "on" : ""}>Копіювати файл</button>
              <button onClick={() => setIngestMode("reference")} className={ingestMode === "reference" ? "on" : ""}>Індексувати файл</button>
            </div>
            <button className="btn primary" onClick={handleSelectAndIngest} disabled={loading}>
              {loading ? "Збереження..." : (ingestMode === "reference" ? "🔗 Індексувати файл" : "📥 Архівувати файл")}
            </button>
            <label className="flabel">
              <input type="checkbox" checked={recursiveIngest} onChange={(e) => setRecursiveIngest(e.target.checked)} />
              Включно з підпапками
            </label>
            <button className="btn" onClick={handleSelectDirectory} disabled={loading}>📁 Архівувати папку</button>
            <button className="btn" onClick={handleReconcile} disabled={loading}>🔎 Перевірити індексацію</button>
            <button className="btn fixity" onClick={handleFullFixity} disabled={loading}>🩺 Перевірити весь архів</button>
            <button className="btn" onClick={async () => {
              try {
                setLoading(true);
                setStatus("🧹 Генерування мініатюр для зображень...");
                const n = await invoke<number>("generate_thumbnails_command");
                setThumbs({});
                setStatus(`✅ Створено мініатюр: ${n}.`);
                if (viewMode === "gallery") fetchThumbnails(objects, true);
              } catch (err: any) {
                setStatus(`Помилка генерації мініатюр: ${String(err)}`);
              } finally { setLoading(false); }
            }} disabled={loading}>🖼 Оновити прев'ю</button>
            <button className="btn" onClick={handleExportCsv} disabled={loading}>📊 Експорт CSV</button>
            <button className="btn" onClick={handleExportJson} disabled={loading}>📋 Експорт JSON</button>
            <button className="btn" onClick={handleCreateBackup} disabled={loading}>💾 Резервна копія</button>
            <button className="btn danger" onClick={handleRestoreBackup} disabled={loading}>♻️ Відновити з копії</button>
            {status && <span className="statusline">{status}</span>}
          </div>

          {/* ЗВІТ FIXITY-ПЕРЕВІРКИ ВСЬОГО АРХІВУ */}
          {fixityReport && (
            <div style={{ background: fixityReport.problems.length ? "#f8d7da" : "#d4edda", border: `1px solid ${fixityReport.problems.length ? "#dc3545" : "#28a745"}`, padding: "12px", borderRadius: "8px", marginBottom: "1.5rem" }}>
              <strong>
                {fixityReport.problems.length === 0
                  ? `✅ Архів цілий: усі ${fixityReport.checked} файлів відповідають контрольним сумам`
                  : `⚠️ Знайдено ${fixityReport.problems.length} проблем(и) з ${fixityReport.checked} перевірених файлів:`}
              </strong>
              {fixityReport.problems.length > 0 && (
                <table style={{ width: "100%", borderCollapse: "collapse", marginTop: "8px", background: "#fff" }}>
                  <thead>
                    <tr>
                      <th style={{ padding: "6px" }}>Документ</th>
                      <th style={{ padding: "6px" }}>Проблема</th>
                      <th style={{ padding: "6px" }}>Шлях</th>
                    </tr>
                  </thead>
                  <tbody>
                    {fixityReport.problems.map(p => (
                      <tr key={p.object_id} style={{ borderBottom: "1px solid #eee" }}>
                        <td style={{ padding: "6px" }}>{p.display_name}</td>
                        <td style={{ padding: "6px" }}>
                          {p.status === "missing" ? "❌ Файл зник" : p.status === "mismatch" ? "⚠️ Сума не збігається (файл пошкоджено або замінено)" : `❌ Не вдалося прочитати: ${p.actual_sha256}`}
                        </td>
                        <td style={{ padding: "6px", fontSize: "0.8rem", wordBreak: "break-all" }}>{p.file_path}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              <button onClick={() => setFixityReport(null)} style={{ marginTop: "8px", padding: "4px 10px", background: "#6c757d", color: "#fff", border: "none", borderRadius: "4px", cursor: "pointer" }}>
                Закрити звіт
              </button>
            </div>
          )}

          {/* ПАНЕЛЬ РОЗШИРЕНОГО ПОШУКУ ТА ФІЛЬТРАЦІЇ */}
          <div className="filterbar">
            <input
              className="search"
              type="text"
              placeholder="Пошук за назвою..."
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
            />
            <select value={filterTagId} onChange={e => setFilterTagId(e.target.value)}>
              <option value="">Всі теги</option>
              {allTags.map(t => <option key={t.id} value={t.id}>#{t.tag_name}</option>)}
            </select>
            <select value={filterPersonId} onChange={e => setFilterPersonId(e.target.value)}>
              <option value="">Всі люди</option>
              {allPersons.map(p => <option key={p.id} value={p.id}>👤 {p.person_name}</option>)}
            </select>
            <select value={filterLocationId} onChange={e => setFilterLocationId(e.target.value)}>
              <option value="">Всі місця</option>
              {allLocations.map(l => <option key={l.id} value={l.id}>📍 {l.location_name}</option>)}
            </select>
            <input
              type="text"
              placeholder="Формат (pdf, jpg...)"
              value={filterExt}
              onChange={e => setFilterExt(e.target.value)}
            />
            <label className="flabel">
              Дата файла від
              <input type="date" value={filterDateFrom} onChange={e => setFilterDateFrom(e.target.value)} />
            </label>
            <label className="flabel">
              до
              <input type="date" value={filterDateTo} onChange={e => setFilterDateTo(e.target.value)} />
            </label>
            <label className="flabel">
              Розмір від (КБ)
              <input type="number" min="0" placeholder="КБ" value={filterMinKb} onChange={e => setFilterMinKb(e.target.value)} />
            </label>
            <label className="flabel">
              до
              <input type="number" min="0" placeholder="КБ" value={filterMaxKb} onChange={e => setFilterMaxKb(e.target.value)} />
            </label>
            <label className="flabel">
              Дата створення від
              <input type="date" value={filterDocDateFrom} onChange={e => setFilterDocDateFrom(e.target.value)} />
            </label>
            <label className="flabel">
              до
              <input type="date" value={filterDocDateTo} onChange={e => setFilterDocDateTo(e.target.value)} />
            </label>
            {(searchQuery || filterTagId || filterPersonId || filterLocationId || filterExt || filterDateFrom || filterDateTo || filterMinKb || filterMaxKb || filterDocDateFrom || filterDocDateTo) && (
              <button
                className="btn ghost"
                onClick={() => { setSearchQuery(""); setFilterTagId(""); setFilterPersonId(""); setFilterLocationId(""); setFilterExt(""); setFilterDateFrom(""); setFilterDateTo(""); setFilterMinKb(""); setFilterMaxKb(""); setFilterDocDateFrom(""); setFilterDocDateTo(""); }}
              >
                Скинути фільтри
              </button>
            )}
          </div>

          <div style={{ display: "flex", gap: "2rem" }}>
            <div style={{ flex: 1.5 }}>
              <h2>Архівовані файли ({totalObjects})</h2>

              <div className="resultsbar">
                <div className="seg">
                  <button onClick={() => setViewMode("table")} className={viewMode === "table" ? "on" : ""}>☰ Таблиця</button>
                  <button onClick={() => setViewMode("gallery")} className={viewMode === "gallery" ? "on" : ""}>🖼 Галерея</button>
                </div>
              </div>

              {objects.length === 0 ? (
                <p className="empty">Нічого не знайдено за заданими критеріями.</p>
              ) : viewMode === "table" ? (
                <table className="objtable">
                  <thead>
                    <tr>
                      <th>Назва</th>
                      <th>Стан</th>
                      <th>Дата архівування</th>
                    </tr>
                  </thead>
                  <tbody>
                    {objects.map((obj) => (
                      <tr key={obj.id} onClick={() => handleRowClick(obj)} className={selectedObject?.id === obj.id ? "sel" : ""}>
                        <td>{obj.display_name}</td>
                        <td>
                          {obj.status === "Active" ? (
                            <span className="pill ok">Архівовано</span>
                          ) : obj.status === "Pending_Deletion" ? (
                            <span className="pill warn">Очікує перегляду</span>
                          ) : obj.status === "Checksum_Error" ? (
                            <span className="pill bad">Пошкоджено</span>
                          ) : (
                            <span className="pill">{obj.status}</span>
                          )}
                        </td>
                        <td className="mono">{new Date(obj.created_at).toLocaleDateString()}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <div className="gallery">
                  {objects.map((obj) => (
                    <div key={obj.id} onClick={() => handleRowClick(obj)} className={"gcard" + (selectedObject?.id === obj.id ? " sel" : "")}>
                      <div className="gthumb">
                        {thumbs[obj.id] ? (
                          <img src={thumbs[obj.id]!} alt={obj.display_name} />
                        ) : (
                          <span title={obj.file_path}>{fileIcon(obj.file_path)}</span>
                        )}
                      </div>
                      <div className="gname" title={obj.display_name}>{obj.display_name}</div>
                      <div className="gmeta">{new Date(obj.created_at).toLocaleDateString()}</div>
                    </div>
                  ))}
                </div>
              )}

              <div className="pager">
                <button onClick={() => setPage(p => p - 1)} disabled={page === 0} className="btn">‹ Назад</button>
                <span>Сторінка {page + 1} з {totalPages}</span>
                <button onClick={() => setPage(p => p + 1)} disabled={page + 1 >= totalPages} className="btn">Вперед ›</button>
              </div>
            </div>
            <div className="inspector">
              <h2>Картка документа</h2>
              {selectedObject ? (
                <div>
                  <p className="kv"><b>Назва:</b> {selectedObject.display_name}</p>
                  <p className="kv"><b>Розмір:</b> {selectedObject.file_size_bytes} байт</p>
                  {selectedObject.storage_type === "reference" && (
                    <div className="callout warn">
                      ⚠️ Зовнішнє посилання — оригінал не гарантовано збережений у сховищі архіву.
                      {!selectedObject.is_accessible && (
                        <strong className="inaccess">
                          ❌ Файл зараз НЕДОСТУПНИЙ за шляхом: {selectedObject.file_path}
                        </strong>
                      )}
                    </div>
                  )}

                  {/* ХЕШ-СУМА ФАЙЛУ */}
                  <div className="kv"><b>Хеш-сума файла:</b></div>
                  <div className="hashbox">{selectedObject.checksum_sha256 || "Не вказано"}</div>

                  <div className="actions">
                    <button className="btn fixity" onClick={async () => {
                      try {
                        const ok = await invoke<boolean>("verify_object_integrity", { objectId: selectedObject.id });
                        setStatus(ok ? "Перевірка цілісності успішна." : "УВАГА: контрольна сума не збігається.");
                        fetchAllData();
                      } catch (err) { setStatus(`Помилка перевірки цілісності: ${String(err)}`); }
                    }}>Перевірити цілісність</button>
                    <button className="btn" onClick={async () => {
                      try { await revealItemInDir(selectedObject.file_path); }
                        catch (err: any) { setStatus(`Не вдалося відкрити теку: ${String(err)}`); }
                    }}>📂 Показати у теці</button>
                  </div>

                  {/* АРХІВНА КАРТКА v2 */}
                  <h4>📋 Архівна картка</h4>
                  {card && (
                    <div className="cardform">
                      <label>Тип файлу
                        <input list="file-kind-options" value={card.file_kind || ""} onChange={e => setCardField("file_kind", e.target.value)} placeholder="Цифрове фото, скан, документ..." />
                      </label>
                      <datalist id="file-kind-options">
                        <option value="Цифрове фото" />
                        <option value="Сканований документ" />
                        <option value="Електронний документ" />
                        <option value="Оцифровлення" />
                      </datalist>
                      <label>Автор / оператор
                        <input value={card.author || ""} onChange={e => setCardField("author", e.target.value)} />
                      </label>
                      <label>Призначення файлу
                        <input value={card.purpose || ""} onChange={e => setCardField("purpose", e.target.value)} />
                      </label>
                      <div className="datarow">
                        <label>Дата створення від
                          <input type="date" value={card.doc_date_from || ""} onChange={e => setCardField("doc_date_from", e.target.value)} />
                        </label>
                        <label>до
                          <input type="date" value={card.doc_date_to || ""} onChange={e => setCardField("doc_date_to", e.target.value)} />
                        </label>
                      </div>
                      <label>Первинне розташування на носії
                        <input value={card.source_location || ""} onChange={e => setCardField("source_location", e.target.value)} placeholder="Disc_7/Scan_2005/" />
                      </label>
                      <button className="btn primary" onClick={handleSaveCard}>💾 Зберегти картку</button>
                    </div>
                  )}

                  <h4>Опис документа</h4>
                  <textarea
                    rows={3}
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    placeholder="Введіть опис документа..."
                  />
                  <div className="actions">
                    <button className="btn" onClick={handleSaveDescription}>Зберегти опис</button>
                  </div>

                  <hr />

                  <h4>Теги</h4>
                  <div className="chipwrap">
                    {tags.map((t, i) => <span key={i} className="chip">#{t}</span>)}
                  </div>
                  <form onSubmit={handleAddTag} className="addform">
                    <input type="text" placeholder="Додати тег..." value={newTag} onChange={(e) => setNewTag(e.target.value)} />
                    <button type="submit" className="btn">+</button>
                  </form>

                  <h4>Люди</h4>
                  <div className="chipwrap">
                    {persons.map((p, i) => <span key={i} className="chip person">👤 {p}</span>)}
                  </div>
                  <form onSubmit={handleAddPerson} className="addform">
                    <input type="text" placeholder="Додати людину..." value={newPerson} onChange={(e) => setNewPerson(e.target.value)} />
                    <button type="submit" className="btn">+</button>
                  </form>

                  <h4>Місця</h4>
                  <div className="chipwrap">
                    {locations.map((l, i) => <span key={i} className="chip location">📍 {l}</span>)}
                  </div>
                  <form onSubmit={handleAddLocation} className="addform">
                    <input type="text" placeholder="Додати місце..." value={newLocation} onChange={(e) => setNewLocation(e.target.value)} />
                    <button type="submit" className="btn">+</button>
                  </form>
                </div>
              ) : <p className="empty">Оберіть документ у списку.</p>}
            </div>
          </div>
        </>
      )}

      {/* ВКЛАДКА: ТЕГИ */}
      {activeTab === "tags" && (
        <div className="panel">
          <h2>Теги</h2>
          <div className="mergepanel">
            <strong>Об'єднання тегів:</strong>
            <select value={mergeSourceId} onChange={e => setMergeSourceId(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Джерело --</option>
              {allTags.map(t => <option key={t.id} value={t.id}>#{t.tag_name}</option>)}
            </select>
            <span>➔ в</span>
            <select value={mergeTargetId} onChange={e => setMergeTargetId(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Ціль --</option>
              {allTags.map(t => <option key={t.id} value={t.id}>#{t.tag_name}</option>)}
            </select>
            <button onClick={handleMergeTags} className="btn">Об'єднати</button>
          </div>
          <table className="objtable">
            <thead><tr><th>Назва</th><th>Файлів</th><th>Дата</th><th style={{ textAlign: "right" }}>Дії</th></tr></thead>
            <tbody>
              {allTags.map(t => (
                <tr key={t.id}>
                  <td>{editingTagId === t.id ? <input value={editingTagName} onChange={e => setEditingTagName(e.target.value)} /> : <strong>#{t.tag_name}</strong>}</td>
                  <td>{t.usage_count}</td>
                  <td style={{ fontSize: "0.85rem" }}>{new Date(t.created_at).toLocaleString()}</td>
                  <td style={{ textAlign: "right" }}>
                    {editingTagId === t.id ? (
                      <>
                        <button onClick={async () => { await invoke("update_tag_name", { tagId: t.id, newName: editingTagName }); setEditingTagId(null); fetchAllData(); fetchFilteredObjects(); }} className="btn primary" style={{ marginRight: "5px" }}>Зберегти</button>
                        <button onClick={() => setEditingTagId(null)} className="btn">Скасувати</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => { setEditingTagId(t.id); setEditingTagName(t.tag_name); }} className="btn" style={{ marginRight: "5px" }}>Редагувати</button>
                        <button onClick={async () => { if (confirm("Видалити тег?")) { await invoke("delete_tag_record", { tagId: t.id }); fetchAllData(); fetchFilteredObjects(); } }} className="btn danger">Видалити</button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ВКЛАДКА: ПЕРСОНАЛІЇ */}
      {activeTab === "persons" && (
        <div className="panel">
          <h2>Люди</h2>
          <div className="mergepanel">
            <strong>Об'єднання людей:</strong>
            <select value={mergePersonSource} onChange={e => setMergePersonSource(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Джерело --</option>
              {allPersons.map(p => <option key={p.id} value={p.id}>👤 {p.person_name}</option>)}
            </select>
            <span>➔ в</span>
            <select value={mergePersonTarget} onChange={e => setMergePersonTarget(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Ціль --</option>
              {allPersons.map(p => <option key={p.id} value={p.id}>👤 {p.person_name}</option>)}
            </select>
            <button onClick={handleMergePersons} className="btn">Об'єднати</button>
          </div>
          <table className="objtable">
            <thead><tr><th>Ім'я</th><th>Файлів</th><th>Дата</th><th style={{ textAlign: "right" }}>Дії</th></tr></thead>
            <tbody>
              {allPersons.map(p => (
                <tr key={p.id}>
                  <td>{editingPersonId === p.id ? <input value={editingPersonName} onChange={e => setEditingPersonName(e.target.value)} /> : <strong>👤 {p.person_name}</strong>}</td>
                  <td>{p.usage_count}</td>
                  <td style={{ fontSize: "0.85rem" }}>{new Date(p.created_at).toLocaleString()}</td>
                  <td style={{ textAlign: "right" }}>
                    {editingPersonId === p.id ? (
                      <>
                        <button onClick={async () => { await invoke("update_person_name", { personId: p.id, newName: editingPersonName }); setEditingPersonId(null); fetchAllData(); fetchFilteredObjects(); }} className="btn primary" style={{ marginRight: "5px" }}>Зберегти</button>
                        <button onClick={() => setEditingPersonId(null)} className="btn">Скасувати</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => { setEditingPersonId(p.id); setEditingPersonName(p.person_name); }} className="btn" style={{ marginRight: "5px" }}>Редагувати</button>
                        <button onClick={async () => { if (confirm("Видалити людину?")) { await invoke("delete_person_record", { personId: p.id }); fetchAllData(); fetchFilteredObjects(); } }} className="btn danger">Видалити</button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ВКЛАДКА: ЛОКАЦІЇ */}
      {activeTab === "locations" && (
        <div className="panel">
          <h2>Місця</h2>
          <div className="mergepanel">
            <strong>Об'єднання місць:</strong>
            <select value={mergeLocationSource} onChange={e => setMergeLocationSource(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Джерело --</option>
              {allLocations.map(l => <option key={l.id} value={l.id}>📍 {l.location_name}</option>)}
            </select>
            <span>➔ в</span>
            <select value={mergeLocationTarget} onChange={e => setMergeLocationTarget(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Ціль --</option>
              {allLocations.map(l => <option key={l.id} value={l.id}>📍 {l.location_name}</option>)}
            </select>
            <button onClick={handleMergeLocations} className="btn">Об'єднати</button>
          </div>
          <table className="objtable">
            <thead><tr><th style={{ padding: "10px" }}>Локація</th><th>Файлів</th><th>Дата</th><th style={{ textAlign: "right" }}>Дії</th></tr></thead>
            <tbody>
              {allLocations.map(l => (
                <tr key={l.id}>
                  <td>{editingLocationId === l.id ? <input value={editingLocationName} onChange={e => setEditingLocationName(e.target.value)} /> : <strong>📍 {l.location_name}</strong>}</td>
                  <td>{l.usage_count}</td>
                  <td style={{ fontSize: "0.85rem" }}>{new Date(l.created_at).toLocaleString()}</td>
                  <td style={{ textAlign: "right" }}>
                    {editingLocationId === l.id ? (
                      <>
                        <button onClick={async () => { await invoke("update_location_name", { locationId: l.id, newName: editingLocationName }); setEditingLocationId(null); fetchAllData(); fetchFilteredObjects(); }} className="btn primary" style={{ marginRight: "5px" }}>Зберегти</button>
                        <button onClick={() => setEditingLocationId(null)} className="btn">Скасувати</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => { setEditingLocationId(l.id); setEditingLocationName(l.location_name); }} className="btn" style={{ marginRight: "5px" }}>Редагувати</button>
                        <button onClick={async () => { if (confirm("Видалити місце?")) { await invoke("delete_location_record", { locationId: l.id }); fetchAllData(); fetchFilteredObjects(); } }} className="btn danger">Видалити</button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ВКЛАДКА: ЕКСПОРТИ */}
      {activeTab === "exports" && (
        <div className="panel">
          <h2>Експорти</h2>
          <p style={{ color: "#666", fontSize: "0.9rem" }}>
            Усі CSV/JSON-таблиці зберігаються у теці програми: ~/.dps-local/exports/
          </p>
          {exportFiles.length === 0 ? (
            <p style={{ color: "#777" }}>Експортів ще немає. Згенеруйте CSV або JSON на вкладці «Реєстр архіву».</p>
          ) : (
            <table className="objtable">
              <thead>
                <tr>
                  <th>Назва</th>
                  <th style={{ padding: "10px" }}>Дата створення</th>
                  <th style={{ padding: "10px" }}>Розмір</th>
                  <th style={{ textAlign: "right" }}>Дії</th>
                </tr>
              </thead>
              <tbody>
                {exportFiles.map(f => (
                  <tr key={f.path} style={{ borderBottom: "1px solid #eee" }}>
                    <td>
                      {renamingExport === f.path ? (
                        <input
                          value={renameValue}
                          onChange={e => setRenameValue(e.target.value)}
                          style={{ padding: "4px", width: "240px" }}
                          autoFocus
                        />
                      ) : <strong>{f.name}</strong>}
                    </td>
                    <td style={{ fontSize: "0.85rem" }}>{new Date(f.modified_at).toLocaleString()}</td>
                    <td style={{ fontSize: "0.85rem" }}>{f.size_bytes} байт</td>
                    <td style={{ textAlign: "right" }}>
                      {renamingExport === f.path ? (
                        <>
                          <button onClick={() => handleRenameExport(f.name)} className="btn primary" style={{ marginRight: "5px" }}>Зберегти</button>
                          <button onClick={() => { setRenamingExport(null); setRenameValue(""); }} className="btn">Скасувати</button>
                        </>
                      ) : (
                        <>
                          <button onClick={() => { setRenamingExport(f.path); setRenameValue(f.name); }} style={{ marginRight: "5px", background: "#ffc107", border: "none", padding: "4px 8px", cursor: "pointer" }}>✏️ Перейменувати</button>
                          <button onClick={() => handleRevealExport(f.path)} style={{ marginRight: "5px", background: "#17a2b8", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>📂 Показати теку</button>
                          <button onClick={() => handleOpenExport(f.path)} style={{ background: "#20c997", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Відкрити</button>
                        </>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {/* ВКЛАДКА: АУДИТ */}
      {activeTab === "audit" && (
        <div className="panel">
          <h2>Журнал дій</h2>
          <p style={{ color: "#666", fontSize: "0.9rem" }}>Незмінний лог операцій для відстеження подій у системі.</p>
          <table className="objtable">
            <thead>
              <tr>
                <th style={{ padding: "10px" }}>Тип дії (Action)</th>
                <th style={{ padding: "10px" }}>Цільовий ID / Деталі</th>
                <th style={{ padding: "10px" }}>Час (UTC)</th>
              </tr>
            </thead>
            <tbody>
              {auditLogs.length === 0 ? (
                <tr><td colSpan={3} style={{ padding: "15px", textAlign: "center", color: "#777" }}>Записи в аудиті відсутні.</td></tr>
              ) : (
                auditLogs.map(log => (
                  <tr key={log.id} style={{ borderBottom: "1px solid #eee" }}>
                    <td>
                      <span style={{ background: "#e2e3e5", padding: "3px 6px", borderRadius: "4px", fontWeight: "bold", fontSize: "0.85rem" }}>
                        {log.action_type}
                      </span>
                    </td>
                    <td style={{ fontSize: "0.85rem" }}>
                      {log.details || log.target_record_id || "—"}
                    </td>
                    <td style={{ padding: "10px", color: "#555", fontSize: "0.85rem" }}>
                      {new Date(log.created_at).toLocaleString()}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}
    </main>
  );
}
