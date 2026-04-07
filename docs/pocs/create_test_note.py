import subprocess

def create_test_note():
    # 建立一則包含標籤的新筆記
    note_title = "Gemini 測試：自動整理機器人回報"
    note_body = "<div>這是一則由 Python 腳本自動生成的測試備忘錄。</div><div>用來測試標籤過濾功能：#Threads</div>"
    
    script = f'''
    tell application "Notes"
        tell account "iCloud" -- 優先嘗試 iCloud 帳號，如果失敗會報錯
            make new note at folder "Notes" with properties {{name:"{note_title}", body:"{note_body}"}}
        end tell
        return "成功建立筆記！"
    end tell
    '''
    
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    
    if stderr:
        # 如果 iCloud 帳號失敗，嘗試預設方式
        script_fallback = f'''
        tell application "Notes"
            make new note with properties {{name:"{note_title}", body:"{note_body}"}}
            return "成功建立筆記 (預設路徑)！"
        end tell
        '''
        process = subprocess.Popen(['osascript', '-e', script_fallback], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        stdout, _ = process.communicate()
        
    return stdout.strip()

if __name__ == "__main__":
    print("正在為您建立測試備忘錄...")
    result = create_test_note()
    print(result)
