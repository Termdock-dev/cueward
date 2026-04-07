import subprocess
import time

def run_applescript(script):
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

def get_notes_from(folder_name):
    # 取得資料夾內所有備忘錄的 ID
    script = f'''
    tell application "Notes"
        set noteList to {{}}
        try
            set targetFolder to folder "{folder_name}"
            set allNotes to notes of targetFolder
            repeat with theNote in allNotes
                set end of noteList to id of theNote
            end repeat
        end try
        
        set oldTIDs to AppleScript's text item delimiters
        set AppleScript's text item delimiters to "|||"
        set output to noteList as text
        set AppleScript's text item delimiters to oldTIDs
        return output
    end tell
    '''
    result = run_applescript(script)
    if not result:
        return []
    return result.split("|||")

def get_note_info(note_id):
    # 透過 ID 取得標題和內文
    script = f'''
    tell application "Notes"
        set theNote to note id "{note_id}"
        set noteName to name of theNote
        set noteBody to body of theNote
        return noteName & "|||" & noteBody
    end tell
    '''
    result = run_applescript(script)
    parts = result.split("|||", 1)
    if len(parts) == 2:
        return parts[0], parts[1]
    return parts[0], ""

def move_note(note_id, target_folder):
    # 執行移動
    script = f'''
    tell application "Notes"
        try
            set theNote to note id "{note_id}"
            set destFolder to folder "{target_folder}"
            move theNote to destFolder
            return "Success"
        on error errStr
            return "Error: " & errStr
        end try
    end tell
    '''
    return run_applescript(script)

def organize_folder(source_folder):
    print(f"🔍 正在掃描 '{source_folder}' 資料夾...")
    note_ids = get_notes_from(source_folder)
    
    if not note_ids or note_ids[0] == "":
        print(f"  此資料夾目前是空的。\n")
        return

    print(f"  找到 {len(note_ids)} 則備忘錄，開始分析...")
    
    moved_count = 0
    # 為了避免在移動時影響列表，我們已經將 ID 存了下來
    for nid in note_ids:
        title, body = get_note_info(nid)
        target = None
        
        title_lower = title.lower()
        body_lower = body.lower()
        
        # 規則判斷
        if "threads" in title_lower or "threads.com" in body_lower or "@" in title_lower:
            target = "threads"
        elif "instagram" in title_lower or "ig" in title_lower:
            target = "收藏"
        elif "idea" in title_lower or "想" in title_lower or "設計" in title_lower or "怎麼做" in title_lower:
            target = "idea"
        elif "ingrelens" in title_lower or "ingrelens" in body_lower:
            target = "INGRELENS"
        elif "http" in body_lower or "www." in body_lower:
            # 如果裡面只有網址，就放進收藏
            target = "收藏"
            
        if target and target != source_folder:
            print(f"  🔄 [{title}]")
            print(f"     ➔ 匹配規則，移動到 '{target}'")
            res = move_note(nid, target)
            if res == "Success":
                moved_count += 1
            else:
                print(f"     ❌ 移動失敗: {res}")
        time.sleep(0.1) # 稍作停頓避免蘋果系統卡住
                
    print(f"✅ '{source_folder}' 整理完成！共自動分類了 {moved_count} 則備忘錄。\n")

if __name__ == "__main__":
    print("🚀 開始執行 Apple Notes 自動分類工具...\n")
    organize_folder("Notes")
    organize_folder("Quick Notes")
