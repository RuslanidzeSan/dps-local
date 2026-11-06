import os

# 1. Шукаємо, де саме лежить SearchFilters
src_dir = "src"
search_mod = "services::archive"
for root, dirs, files in os.walk(src_dir):
    for f in files:
        if f.endswith(".rs"):
            path = os.path.join(root, f)
            with open(path, 'r', encoding='utf-8') as file:
                if "pub struct SearchFilters" in file.read():
                    rel = os.path.relpath(path, src_dir)
                    mod = rel.replace(".rs", "").replace("/", "::")
                    if mod.endswith("::mod"): mod = mod[:-5]
                    if mod == "lib": mod = ""
                    search_mod = mod
                    break

prefix = f"dps_local_lib::{search_mod}::" if search_mod else "dps_local_lib::"

# 2. Збираємо правильний код тесту
test_code = f"""
#[test]
fn search_filters_by_attribute_value() {{
    let mut conn = fresh_db();
    insert_object(&conn, "o1");
    insert_object(&conn, "o2");

    attrs::add_attr(&mut conn, "Об'єктив", "select", None, Some("28mm; 50mm")).unwrap();
    attrs::add_attr(&mut conn, "Дата зйомки", "date", None, None).unwrap();
    let ids = attrs::get_all_attrs_with_stats(&conn).unwrap();
    let id_lens = ids.iter().find(|a| a.name == "Об'єктив").unwrap().id.clone();
    let id_date = ids.iter().find(|a| a.name == "Дата зйомки").unwrap().id.clone();

    attrs::set_object_attr_value(&conn, "o1", &id_lens, Some("50mm")).unwrap();
    attrs::set_object_attr_value(&conn, "o2", &id_date, Some("1974-07-06")).unwrap(); 

    let r = {prefix}search_archive_objects_filtered(&conn, &{prefix}SearchFilters {{
        attr_id: Some(id_lens.clone()), attr_text: Some("50".into()), ..Default::default()
    }}).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].id, "o1");

    let r = {prefix}search_archive_objects_filtered(&conn, &{prefix}SearchFilters {{
        attr_id: Some(id_date.clone()), attr_text: Some("1974".into()), ..Default::default()
    }}).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].id, "o2");

    assert!({prefix}search_archive_objects_filtered(&conn, &{prefix}SearchFilters {{
        attr_id: Some(id_lens.clone()), attr_text: Some("85mm".into()), ..Default::default()
    }}).unwrap().is_empty());

    let r = {prefix}search_archive_objects_filtered(&conn, &{prefix}SearchFilters {{
        attr_id: Some(id_lens), attr_text: None, ..Default::default()
    }}).unwrap();
    assert_eq!(r.len(), 2);
}}
"""

tp = "tests/attr_values_test.rs"
with open(tp, 'r', encoding='utf-8') as f:
    content = f.read()

# 3. Видаляємо зламаний тест, якщо він є, і дописуємо новий
idx = content.find("#[test]\nfn search_filters_by_attribute_value")
if idx != -1:
    content = content[:idx]

with open(tp, 'w', encoding='utf-8') as f:
    f.write(content.strip() + "\n\n" + test_code.strip() + "\n")

print(f"✅ Тест успішно виправлено! (Використано модуль {prefix})")
