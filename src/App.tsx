import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

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

interface AuditLogDTO {
  id: string;
  action_type: string;
  target_record_id: string | null;
  details: string | null;
  created_at: string;
}
interface IngestResult { object_id: string; duplicate_detected: boolean; }

export default function App() {
  const [activeTab, setActiveTab] = useState<"archive" | "tags" | "persons" | "locations" | "audit">("archive");

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

  // Фільтри реєстру
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [filterTagId, setFilterTagId] = useState<string>("");
  const [filterPersonId, setFilterPersonId] = useState<string>("");
  const [filterLocationId, setFilterLocationId] = useState<string>("");

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

  const fetchFilteredObjects = async () => {
    try {
      const res = await invoke<ArchiveObject[]>("search_objects_filtered", {
        query: searchQuery || null,
        tagId: filterTagId || null,
        personId: filterPersonId || null,
        locationId: filterLocationId || null,
      });
      setObjects(res);
      // Оновлюємо вибраний об'єкт, якщо він змінився
      if (selectedObject) {
        const updated = res.find(o => o.id === selectedObject.id);
        if (updated) setSelectedObject(updated);
      }
    } catch (err) {
      console.error("Помилка пошуку:", err);
    }
  };

  const fetchMetadata = async (objectId: string) => {
    try {
      setTags(await invoke<string[]>("get_object_tags", { objectId }));
      setPersons(await invoke<string[]>("get_object_persons", { objectId }));
      setLocations(await invoke<string[]>("get_object_locations", { objectId }));
    } catch (err) { console.error(err); }
  };

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
  }, [searchQuery, filterTagId, filterPersonId, filterLocationId]);

  const handleRowClick = (obj: ArchiveObject) => {
    setSelectedObject(obj);
    setDescription(obj.description || "");
    fetchMetadata(obj.id);
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

  // Експорт CSV: результат від бекенду → файл через Blob
  const handleExportCsv = async () => {
    try {
      const csv = await invoke<string>("export_objects_csv_command");
      const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `archive_export_${new Date().toISOString().slice(0, 10)}.csv`;
      link.click();
      URL.revokeObjectURL(url);
      setStatus("Експорт CSV завершено!");
    } catch (err: any) {
      setStatus(`Помилка експорту CSV: ${String(err)}`);
    }
  };

  // Експорт JSON: результат від бекенду → файл через Blob
  const handleExportJson = async () => {
    try {
      const json = await invoke<string>("export_objects_json_command");
      const blob = new Blob([json], { type: "application/json;charset=utf-8;" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `archive_export_${new Date().toISOString().slice(0, 10)}.json`;
      link.click();
      URL.revokeObjectURL(url);
      setStatus("Експорт JSON завершено!");
    } catch (err: any) {
      setStatus(`Помилка експорту JSON: ${String(err)}`);
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
    <main style={{ padding: "2rem", fontFamily: "sans-serif", maxWidth: "1250px", margin: "0 auto" }}>
      <h1>dps-local: Цифровий Архів</h1>
      <p>Локальна автономна система архівування файлів</p>

      {/* Верхні вкладки */}
      <div style={{ display: "flex", gap: "0.5rem", margin: "1.5rem 0", borderBottom: "2px solid #ddd", paddingBottom: "0.5rem", flexWrap: "wrap" }}>
        <button onClick={() => setActiveTab("archive")} style={{ padding: "0.5rem 1rem", background: activeTab === "archive" ? "#007bff" : "#f1f1f1", color: activeTab === "archive" ? "#fff" : "#333", border: "none", borderRadius: "4px", cursor: "pointer", fontWeight: "bold" }}>📂 Реєстр архіву</button>
        <button onClick={() => { setActiveTab("tags"); fetchAllData(); }} style={{ padding: "0.5rem 1rem", background: activeTab === "tags" ? "#007bff" : "#f1f1f1", color: activeTab === "tags" ? "#fff" : "#333", border: "none", borderRadius: "4px", cursor: "pointer", fontWeight: "bold" }}>🏷️ Довідник тегів</button>
        <button onClick={() => { setActiveTab("persons"); fetchAllData(); }} style={{ padding: "0.5rem 1rem", background: activeTab === "persons" ? "#007bff" : "#f1f1f1", color: activeTab === "persons" ? "#fff" : "#333", border: "none", borderRadius: "4px", cursor: "pointer", fontWeight: "bold" }}>👤 Довідник осіб</button>
        <button onClick={() => { setActiveTab("locations"); fetchAllData(); }} style={{ padding: "0.5rem 1rem", background: activeTab === "locations" ? "#007bff" : "#f1f1f1", color: activeTab === "locations" ? "#fff" : "#333", border: "none", borderRadius: "4px", cursor: "pointer", fontWeight: "bold" }}>📍 Довідник локацій</button>
        <button onClick={() => { setActiveTab("audit"); fetchAllData(); }} style={{ padding: "0.5rem 1rem", background: activeTab === "audit" ? "#007bff" : "#f1f1f1", color: activeTab === "audit" ? "#fff" : "#333", border: "none", borderRadius: "4px", cursor: "pointer", fontWeight: "bold" }}>📜 Журнал аудиту</button>
      </div>

      {activeTab === "archive" && (
        <>
          {/* ПАНЕЛЬ ДІЙ НАД АРХІВОМ */}
          <div style={{ marginBottom: "1rem", display: "flex", gap: "10px", alignItems: "center", flexWrap: "wrap" }}>
            <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
              <button
                onClick={() => setIngestMode("managed")}
                style={{ padding: "0.5rem 0.8rem", background: ingestMode === "managed" ? "#28a745" : "#e9ecef", color: ingestMode === "managed" ? "#fff" : "#333", border: "1px solid #ccc", borderRadius: "4px", cursor: "pointer" }}
              >Режим: копія в архів</button>
              <button
                onClick={() => setIngestMode("reference")}
                style={{ padding: "0.5rem 0.8rem", background: ingestMode === "reference" ? "#fd7e14" : "#e9ecef", color: ingestMode === "reference" ? "#fff" : "#333", border: "1px solid #ccc", borderRadius: "4px", cursor: "pointer" }}
              >Режим: лише посилання</button>
            </div>
            <button onClick={handleSelectAndIngest} disabled={loading} style={{ padding: "0.6rem 1.2rem", fontSize: "1rem", cursor: "pointer", background: ingestMode === "reference" ? "#fd7e14" : "#28a745", color: "#fff", border: "none", borderRadius: "4px" }}>
              {loading ? "Збереження..." : (ingestMode === "reference" ? "🔗 Проіндексувати посиланням" : "📥 Завантажити файл в архів")}
            </button>
            <label style={{ display: "flex", alignItems: "center", gap: "5px", fontSize: "0.9rem", cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={recursiveIngest}
                onChange={(e) => setRecursiveIngest(e.target.checked)}
              />
              Включно з підпапками
            </label>
            <button onClick={handleSelectDirectory} disabled={loading} style={{ padding: "0.6rem 1rem", cursor: "pointer", background: "#6f42c1", color: "#fff", border: "none", borderRadius: "4px" }}>
              📁 Імпортувати каталог
            </button>
            <button onClick={handleReconcile} disabled={loading} style={{ padding: "0.6rem 1rem", cursor: "pointer", background: "#17a2b8", color: "#fff", border: "none", borderRadius: "4px" }}>
              🔎 Перевірити посилання
            </button>
            <button onClick={handleExportCsv} disabled={loading} style={{ padding: "0.6rem 1rem", cursor: "pointer", background: "#20c997", color: "#fff", border: "none", borderRadius: "4px" }}>
              📊 Експорт CSV
            </button>
            <button onClick={handleExportJson} disabled={loading} style={{ padding: "0.6rem 1rem", cursor: "pointer", background: "#6610f2", color: "#fff", border: "none", borderRadius: "4px" }}>
              📋 Експорт JSON
            </button>
            <button onClick={handleCreateBackup} disabled={loading} style={{ padding: "0.6rem 1rem", cursor: "pointer", background: "#0d6efd", color: "#fff", border: "none", borderRadius: "4px" }}>
              💾 Резервна копія
            </button>
            <button onClick={handleRestoreBackup} disabled={loading} style={{ padding: "0.6rem 1rem", cursor: "pointer", background: "#dc3545", color: "#fff", border: "none", borderRadius: "4px" }}>
              ♻️ Відновити з копії
            </button>
            {status && <span style={{ color: "#555" }}>{status}</span>}
          </div>

          {/* ПАНЕЛЬ РОЗШИРЕНОГО ПОШУКУ ТА ФІЛЬТРАЦІЇ */}
          <div style={{ background: "#eef2f5", padding: "12px", borderRadius: "8px", marginBottom: "1.5rem", display: "flex", gap: "10px", flexWrap: "wrap", alignItems: "center" }}>
            <strong>🔍 Фільтри реєстру:</strong>
            <input 
              type="text" 
              placeholder="Пошук за назвою..." 
              value={searchQuery} 
              onChange={e => setSearchQuery(e.target.value)} 
              style={{ padding: "6px", width: "220px", borderRadius: "4px", border: "1px solid #ccc" }}
            />
            <select value={filterTagId} onChange={e => setFilterTagId(e.target.value)} style={{ padding: "6px", borderRadius: "4px" }}>
              <option value="">Всі теги</option>
              {allTags.map(t => <option key={t.id} value={t.id}>#{t.tag_name}</option>)}
            </select>
            <select value={filterPersonId} onChange={e => setFilterPersonId(e.target.value)} style={{ padding: "6px", borderRadius: "4px" }}>
              <option value="">Всі особи</option>
              {allPersons.map(p => <option key={p.id} value={p.id}>👤 {p.person_name}</option>)}
            </select>
            <select value={filterLocationId} onChange={e => setFilterLocationId(e.target.value)} style={{ padding: "6px", borderRadius: "4px" }}>
              <option value="">Всі локації</option>
              {allLocations.map(l => <option key={l.id} value={l.id}>📍 {l.location_name}</option>)}
            </select>
            {(searchQuery || filterTagId || filterPersonId || filterLocationId) && (
              <button 
                onClick={() => { setSearchQuery(""); setFilterTagId(""); setFilterPersonId(""); setFilterLocationId(""); }}
                style={{ padding: "6px 10px", background: "#6c757d", color: "#fff", border: "none", borderRadius: "4px", cursor: "pointer" }}
              >
                Скинути фільтри
              </button>
            )}
          </div>

          <div style={{ display: "flex", gap: "2rem" }}>
            <div style={{ flex: 1.5 }}>
              <h2>Список документів ({objects.length})</h2>
              {objects.length === 0 ? <p style={{ color: "#777" }}>Нічого не знайдено за заданими критеріями.</p> : (
                <table style={{ width: "100%", borderCollapse: "collapse", marginTop: "1rem", textAlign: "left", background: "#fff" }}>
                  <thead>
                    <tr style={{ borderBottom: "2px solid #ccc", background: "#f9f9f9" }}>
                      <th style={{ padding: "8px" }}>Назва</th>
                      <th style={{ padding: "8px" }}>Статус</th>
                      <th style={{ padding: "8px" }}>Дата</th>
                    </tr>
                  </thead>
                  <tbody>
                    {objects.map((obj) => (
                      <tr key={obj.id} onClick={() => handleRowClick(obj)} style={{ borderBottom: "1px solid #eee", cursor: "pointer", background: selectedObject?.id === obj.id ? "#e6f2ff" : "transparent" }}>
                        <td style={{ padding: "8px" }}>{obj.display_name}</td>
                        <td style={{ padding: "8px" }}>{obj.status}</td>
                        <td style={{ padding: "8px" }}>{new Date(obj.created_at).toLocaleDateString()}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            <div style={{ flex: 1.2, background: "#f9f9f9", padding: "1.5rem", borderRadius: "8px", border: "1px solid #ddd" }}>
              <h2>Картка документа</h2>
              {selectedObject ? (
                <div>
                  <p><strong>Назва:</strong> {selectedObject.display_name}</p>
                  <p><strong>Розмір:</strong> {selectedObject.file_size_bytes} байт</p>
                  {selectedObject.storage_type === "reference" && (
                    <p style={{ background: "#fff3cd", border: "1px solid #ffc107", padding: "6px 8px", borderRadius: "4px", fontSize: "0.85rem" }}>
                      ⚠️ Зовнішнє посилання — оригінал не гарантовано збережений у сховищі архіву.
                      {!selectedObject.is_accessible && (
                        <strong style={{ color: "#dc3545", display: "block", marginTop: "4px" }}>
                          ❌ Файл зараз НЕДОСТУПНИЙ за шляхом: {selectedObject.file_path}
                        </strong>
                      )}
                    </p>
                  )}
                  {/* ХЕШ-СУМА ФАЙЛУ */}
                  <div style={{ marginTop: "0.5rem", marginBottom: "1rem", wordBreak: "break-all" }}>
                    <strong style={{ fontSize: "0.85rem", color: "#444" }}>Хеш-сума файла:</strong>
                    <div style={{ background: "#eee", padding: "6px", borderRadius: "4px", fontSize: "0.75rem", fontFamily: "monospace", color: "#333", marginTop: "2px" }}>
                      {selectedObject.checksum_sha256 || "Не вказано"}
                    </div>
                  </div>
                  <button onClick={async () => {
                    try {
                      const ok = await invoke<boolean>("verify_object_integrity", { objectId: selectedObject.id });
                      setStatus(ok ? "Перевірка цілісності успішна." : "УВАГА: контрольна сума не збігається.");
                      fetchAllData();
                    } catch (err) { setStatus(`Помилка перевірки цілісності: ${String(err)}`); }
                  }} style={{ padding: "4px 10px", background: "#6f42c1", color: "#fff", border: "none", borderRadius: "4px", cursor: "pointer", marginBottom: "1rem", fontSize: "0.85rem" }}>
                    Перевірити цілісність
                  </button>

                  {/* ДОВІЛЬНИЙ ОПИС */}
                  <h4 style={{ marginTop: "1rem" }}>Довільний опис документа</h4>
                  <textarea 
                    rows={3}
                    value={description} 
                    onChange={(e) => setDescription(e.target.value)} 
                    placeholder="Введіть довільний опис документа..."
                    style={{ width: "100%", padding: "6px", borderRadius: "4px", border: "1px solid #ccc", marginBottom: "5px", fontSize: "0.9rem" }}
                  />
                  <button onClick={handleSaveDescription} style={{ padding: "4px 10px", background: "#007bff", color: "#fff", border: "none", borderRadius: "4px", cursor: "pointer", marginBottom: "1rem", fontSize: "0.85rem" }}>
                    Зберегти опис
                  </button>

                  <hr style={{ margin: "0.5rem 0 1rem 0", borderColor: "#ccc" }} />

                  <h4>Теги</h4>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "5px", marginBottom: "0.5rem" }}>
                    {tags.map((t, i) => <span key={i} style={{ background: "#e0e0e0", padding: "2px 6px", borderRadius: "4px", fontSize: "0.8rem" }}>#{t}</span>)}
                  </div>
                  <form onSubmit={handleAddTag} style={{ display: "flex", gap: "5px", marginBottom: "1rem" }}>
                    <input type="text" placeholder="Додати тег..." value={newTag} onChange={(e) => setNewTag(e.target.value)} style={{ padding: "4px", flex: 1 }} />
                    <button type="submit" style={{ padding: "4px 8px" }}>+</button>
                  </form>

                  <h4>Персоналії</h4>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "5px", marginBottom: "0.5rem" }}>
                    {persons.map((p, i) => <span key={i} style={{ background: "#d1e7dd", padding: "2px 6px", borderRadius: "4px", fontSize: "0.8rem" }}>👤 {p}</span>)}
                  </div>
                  <form onSubmit={handleAddPerson} style={{ display: "flex", gap: "5px", marginBottom: "1rem" }}>
                    <input type="text" placeholder="Додати особу..." value={newPerson} onChange={(e) => setNewPerson(e.target.value)} style={{ padding: "4px", flex: 1 }} />
                    <button type="submit" style={{ padding: "4px 8px" }}>+</button>
                  </form>

                  <h4>Локації</h4>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "5px", marginBottom: "0.5rem" }}>
                    {locations.map((l, i) => <span key={i} style={{ background: "#cfe2ff", padding: "2px 6px", borderRadius: "4px", fontSize: "0.8rem" }}>📍 {l}</span>)}
                  </div>
                  <form onSubmit={handleAddLocation} style={{ display: "flex", gap: "5px" }}>
                    <input type="text" placeholder="Додати локацію..." value={newLocation} onChange={(e) => setNewLocation(e.target.value)} style={{ padding: "4px", flex: 1 }} />
                    <button type="submit" style={{ padding: "4px 8px" }}>+</button>
                  </form>
                </div>
              ) : <p style={{ color: "#777" }}>Оберіть документ у списку.</p>}
            </div>
          </div>
        </>
      )}

      {/* ВКЛАДКА: ТЕГИ */}
      {activeTab === "tags" && (
        <div style={{ background: "#f9f9f9", padding: "1.5rem", borderRadius: "8px", border: "1px solid #ddd" }}>
          <h2>Довідник тегів (`ref_tags`)</h2>
          <div style={{ background: "#eef2f5", padding: "10px", borderRadius: "6px", margin: "1rem 0", display: "flex", gap: "10px", alignItems: "center", flexWrap: "wrap" }}>
            <strong>Злиття тегів:</strong>
            <select value={mergeSourceId} onChange={e => setMergeSourceId(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Джерело --</option>
              {allTags.map(t => <option key={t.id} value={t.id}>#{t.tag_name}</option>)}
            </select>
            <span>➔ в</span>
            <select value={mergeTargetId} onChange={e => setMergeTargetId(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Ціль --</option>
              {allTags.map(t => <option key={t.id} value={t.id}>#{t.tag_name}</option>)}
            </select>
            <button onClick={handleMergeTags} style={{ background: "#6f42c1", color: "#fff", border: "none", padding: "6px 12px", cursor: "pointer", borderRadius: "4px" }}>Злити</button>
          </div>
          <table style={{ width: "100%", borderCollapse: "collapse", background: "#fff", marginTop: "1rem" }}>
            <thead><tr style={{ background: "#f1f1f1", textAlign: "left" }}><th style={{ padding: "10px" }}>Назва</th><th style={{ padding: "10px" }}>Usage Count</th><th style={{ padding: "10px" }}>Дата</th><th style={{ padding: "10px", textAlign: "right" }}>Дії</th></tr></thead>
            <tbody>
              {allTags.map(t => (
                <tr key={t.id} style={{ borderBottom: "1px solid #eee" }}>
                  <td style={{ padding: "10px" }}>{editingTagId === t.id ? <input value={editingTagName} onChange={e => setEditingTagName(e.target.value)} /> : <strong>#{t.tag_name}</strong>}</td>
                  <td style={{ padding: "10px" }}>{t.usage_count} док.</td>
                  <td style={{ padding: "10px", fontSize: "0.85rem" }}>{new Date(t.created_at).toLocaleString()}</td>
                  <td style={{ padding: "10px", textAlign: "right" }}>
                    {editingTagId === t.id ? (
                      <>
                        <button onClick={async () => { await invoke("update_tag_name", { tagId: t.id, newName: editingTagName }); setEditingTagId(null); fetchAllData(); fetchFilteredObjects(); }} style={{ marginRight: "5px", background: "#28a745", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Зберегти</button>
                        <button onClick={() => setEditingTagId(null)} style={{ background: "#6c757d", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Скасувати</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => { setEditingTagId(t.id); setEditingTagName(t.tag_name); }} style={{ marginRight: "5px", background: "#ffc107", border: "none", padding: "4px 8px", cursor: "pointer" }}>Редагувати</button>
                        <button onClick={async () => { if (confirm("Видалити тег?")) { await invoke("delete_tag_record", { tagId: t.id }); fetchAllData(); fetchFilteredObjects(); } }} style={{ background: "#dc3545", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Видалити</button>
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
        <div style={{ background: "#f9f9f9", padding: "1.5rem", borderRadius: "8px", border: "1px solid #ddd" }}>
          <h2>Довідник персоналій (`ref_persons`)</h2>
          <div style={{ background: "#eef2f5", padding: "10px", borderRadius: "6px", margin: "1rem 0", display: "flex", gap: "10px", alignItems: "center", flexWrap: "wrap" }}>
            <strong>Злиття осіб:</strong>
            <select value={mergePersonSource} onChange={e => setMergePersonSource(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Джерело --</option>
              {allPersons.map(p => <option key={p.id} value={p.id}>👤 {p.person_name}</option>)}
            </select>
            <span>➔ в</span>
            <select value={mergePersonTarget} onChange={e => setMergePersonTarget(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Ціль --</option>
              {allPersons.map(p => <option key={p.id} value={p.id}>👤 {p.person_name}</option>)}
            </select>
            <button onClick={handleMergePersons} style={{ background: "#6f42c1", color: "#fff", border: "none", padding: "6px 12px", cursor: "pointer", borderRadius: "4px" }}>Злити</button>
          </div>
          <table style={{ width: "100%", borderCollapse: "collapse", background: "#fff", marginTop: "1rem" }}>
            <thead><tr style={{ background: "#f1f1f1", textAlign: "left" }}><th style={{ padding: "10px" }}>Ім'я</th><th style={{ padding: "10px" }}>Usage Count</th><th style={{ padding: "10px" }}>Дата</th><th style={{ padding: "10px", textAlign: "right" }}>Дії</th></tr></thead>
            <tbody>
              {allPersons.map(p => (
                <tr key={p.id} style={{ borderBottom: "1px solid #eee" }}>
                  <td style={{ padding: "10px" }}>{editingPersonId === p.id ? <input value={editingPersonName} onChange={e => setEditingPersonName(e.target.value)} /> : <strong>👤 {p.person_name}</strong>}</td>
                  <td style={{ padding: "10px" }}>{p.usage_count} док.</td>
                  <td style={{ padding: "10px", fontSize: "0.85rem" }}>{new Date(p.created_at).toLocaleString()}</td>
                  <td style={{ padding: "10px", textAlign: "right" }}>
                    {editingPersonId === p.id ? (
                      <>
                        <button onClick={async () => { await invoke("update_person_name", { personId: p.id, newName: editingPersonName }); setEditingPersonId(null); fetchAllData(); fetchFilteredObjects(); }} style={{ marginRight: "5px", background: "#28a745", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Зберегти</button>
                        <button onClick={() => setEditingPersonId(null)} style={{ background: "#6c757d", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Скасувати</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => { setEditingPersonId(p.id); setEditingPersonName(p.person_name); }} style={{ marginRight: "5px", background: "#ffc107", border: "none", padding: "4px 8px", cursor: "pointer" }}>Редагувати</button>
                        <button onClick={async () => { if (confirm("Видалити особу?")) { await invoke("delete_person_record", { personId: p.id }); fetchAllData(); fetchFilteredObjects(); } }} style={{ background: "#dc3545", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Видалити</button>
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
        <div style={{ background: "#f9f9f9", padding: "1.5rem", borderRadius: "8px", border: "1px solid #ddd" }}>
          <h2>Довідник локацій (`ref_locations`)</h2>
          <div style={{ background: "#eef2f5", padding: "10px", borderRadius: "6px", margin: "1rem 0", display: "flex", gap: "10px", alignItems: "center", flexWrap: "wrap" }}>
            <strong>Злиття локацій:</strong>
            <select value={mergeLocationSource} onChange={e => setMergeLocationSource(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Джерело --</option>
              {allLocations.map(l => <option key={l.id} value={l.id}>📍 {l.location_name}</option>)}
            </select>
            <span>➔ в</span>
            <select value={mergeLocationTarget} onChange={e => setMergeLocationTarget(e.target.value)} style={{ padding: "5px" }}>
              <option value="">-- Ціль --</option>
              {allLocations.map(l => <option key={l.id} value={l.id}>📍 {l.location_name}</option>)}
            </select>
            <button onClick={handleMergeLocations} style={{ background: "#6f42c1", color: "#fff", border: "none", padding: "6px 12px", cursor: "pointer", borderRadius: "4px" }}>Злити</button>
          </div>
          <table style={{ width: "100%", borderCollapse: "collapse", background: "#fff", marginTop: "1rem" }}>
            <thead><tr style={{ background: "#f1f1f1", textAlign: "left" }}><th style={{ padding: "10px" }}>Локація</th><th style={{ padding: "10px" }}>Usage Count</th><th style={{ padding: "10px" }}>Дата</th><th style={{ padding: "10px", textAlign: "right" }}>Дії</th></tr></thead>
            <tbody>
              {allLocations.map(l => (
                <tr key={l.id} style={{ borderBottom: "1px solid #eee" }}>
                  <td style={{ padding: "10px" }}>{editingLocationId === l.id ? <input value={editingLocationName} onChange={e => setEditingLocationName(e.target.value)} /> : <strong>📍 {l.location_name}</strong>}</td>
                  <td style={{ padding: "10px" }}>{l.usage_count} док.</td>
                  <td style={{ padding: "10px", fontSize: "0.85rem" }}>{new Date(l.created_at).toLocaleString()}</td>
                  <td style={{ padding: "10px", textAlign: "right" }}>
                    {editingLocationId === l.id ? (
                      <>
                        <button onClick={async () => { await invoke("update_location_name", { locationId: l.id, newName: editingLocationName }); setEditingLocationId(null); fetchAllData(); fetchFilteredObjects(); }} style={{ marginRight: "5px", background: "#28a745", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Зберегти</button>
                        <button onClick={() => setEditingLocationId(null)} style={{ background: "#6c757d", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Скасувати</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => { setEditingLocationId(l.id); setEditingLocationName(l.location_name); }} style={{ marginRight: "5px", background: "#ffc107", border: "none", padding: "4px 8px", cursor: "pointer" }}>Редагувати</button>
                        <button onClick={async () => { if (confirm("Видалити локацію?")) { await invoke("delete_location_record", { locationId: l.id }); fetchAllData(); fetchFilteredObjects(); } }} style={{ background: "#dc3545", color: "#fff", border: "none", padding: "4px 8px", cursor: "pointer" }}>Видалити</button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ВКЛАДКА: АУДИТ */}
      {activeTab === "audit" && (
        <div style={{ background: "#f9f9f9", padding: "1.5rem", borderRadius: "8px", border: "1px solid #ddd" }}>
          <h2>📜 Журнал аудиту системи (`audit_events`)</h2>
          <p style={{ color: "#666", fontSize: "0.9rem" }}>Незмінний лог операцій для відстеження подій у системі.</p>
          <table style={{ width: "100%", borderCollapse: "collapse", background: "#fff", marginTop: "1rem" }}>
            <thead>
              <tr style={{ background: "#f1f1f1", textAlign: "left" }}>
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
                    <td style={{ padding: "10px" }}>
                      <span style={{ background: "#e2e3e5", padding: "3px 6px", borderRadius: "4px", fontWeight: "bold", fontSize: "0.85rem" }}>
                        {log.action_type}
                      </span>
                    </td>
                    <td style={{ padding: "10px", fontSize: "0.85rem" }}>
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
