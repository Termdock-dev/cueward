import subprocess
import time

def run_applescript(script):
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

def cleanup_notes():
    print("🧹 正在啟動清理機器人，修復重複標籤問題...")
    
    # 取得所有筆記 ID
    script_get_all = 'tell application "Notes" to get id of every note'
    ids_str = run_applescript(script_get_all)
    if not ids_str: return
    ids = [i.strip() for i in ids_str.split(",")]

    for nid in ids:
        # 取得內容
        body = run_applescript(f'tell application "Notes" to get body of note id "{nid}"')
        
        # 如果發現重複的標籤 (多於 1 個)
        tag = "#WebArchive"
        if body.count(tag) > 1:
            print(f"🛠️ 正在修復筆記: {nid[:10]}...")
            
            # 移除所有該標籤
            clean_body = body.replace(tag, "")
            # 只在最後面加回一個 (包在 div 裡)
            final_body = clean_body + f"<div>{tag}</div>"
            
            # 更新回備忘錄
            escaped_body = final_body.replace('"', '\\"')
            run_applescript(f'tell application "Notes" to set body of note id "{nid}" to "{escaped_body}"')
            
    print("\n✅ 清理完成！重複的標籤應該都消失了。")

if __name__ == "__main__":
    cleanup_notes()
