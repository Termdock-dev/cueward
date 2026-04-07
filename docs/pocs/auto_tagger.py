import subprocess
import time

def run_applescript(script):
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

def get_all_notes():
    # 取得所有備忘錄的 ID
    script = '''
    tell application "Notes"
        set noteList to {}
        set allNotes to notes
        repeat with theNote in allNotes
            set end of noteList to id of theNote
        end repeat
        
        set oldTIDs to AppleScript's text item delimiters
        set AppleScript's text item delimiters to "|||"
        set output to noteList as text
        set AppleScript's text item delimiters to oldTIDs
        return output
    end tell
    '''
    result = run_applescript(script)
    if not result: return []
    return result.split("|||")

def get_note_content(note_id):
    script = f'tell application "Notes" to get body of note id "{note_id}"'
    return run_applescript(script)

def add_tag_to_note(note_id, current_body, tag):
    # 如果標籤已經存在，就不重複添加
    if tag in current_body:
        return "Already Tagged"
        
    # 在 HTML 內容的結尾（</div> 之前）插入標籤
    new_body = current_body.replace("</div>", f" {tag}</div>", -1) 
    # 如果沒有 </div>，就直接加在最後面
    if new_body == current_body:
        new_body = current_body + f"<div>{tag}</div>"

    # 使用 AppleScript 更新內容
    # 注意：處理 HTML 內容時需要小心引號
    escaped_body = new_body.replace('"', '\\"')
    script = f'''
    tell application "Notes"
        set theNote to note id "{note_id}"
        set body of theNote to "{escaped_body}"
        return "Success"
    end tell
    '''
    return run_applescript(script)

def start_tagging():
    print("🏷️ 正在啟動自動標籤引擎...")
    note_ids = get_all_notes()
    print(f"📋 掃描到 {len(note_ids)} 則備忘錄。\n")
    
    tagged_count = 0
    
    for nid in note_ids:
        body = get_note_content(nid)
        body_lower = body.lower()
        
        tags_to_add = []
        
        # 判斷規則
        if "threads.net" in body_lower or "threads.com" in body_lower:
            tags_to_add.append("#Threads")
        elif "instagram.com" in body_lower or "ig.me" in body_lower:
            tags_to_add.append("#Instagram")
        elif "http" in body_lower and not any(x in body_lower for x in ["threads", "instagram"]):
            tags_to_add.append("#WebArchive")
            
        if tags_to_add:
            for tag in tags_to_add:
                res = add_tag_to_note(nid, body, tag)
                if res == "Success":
                    print(f"✅ 已為筆記標記 {tag} (ID: {nid[:10]}...)")
                    tagged_count += 1
                    # 更新 body 以免重複標記
                    body = get_note_content(nid)
        
        time.sleep(0.05)

    print(f"\n✨ 完成！共處理了 {tagged_count} 個新標籤。")
    print("現在請查看你的 Apple Notes，智慧型資料夾應該已經自動更新了！")

if __name__ == "__main__":
    start_tagging()
