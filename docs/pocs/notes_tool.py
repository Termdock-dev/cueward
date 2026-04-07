import subprocess
import argparse
import sys

def run_applescript(script):
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

def list_folders():
    script = 'tell application "Notes" to get name of every folder'
    print(f"資料夾清單: {run_applescript(script)}")

def search_notes(keyword):
    script = f'''
    tell application "Notes"
        set noteList to {{}}
        set foundNotes to (notes whose body contains "{keyword}" or name contains "{keyword}")
        repeat with theNote in foundNotes
            set end of noteList to (name of theNote)
        end repeat
        return noteList
    end tell
    '''
    print(f"搜尋結果: {run_applescript(script)}")

def get_note(title):
    script = f'''
    tell application "Notes"
        try
            set theNote to note "{title}"
            return body of theNote
        on error
            return "錯誤: 找不到筆記 '{title}'"
        end try
    end tell
    '''
    print(run_applescript(script))

def create_note(title, content, folder="Notes"):
    script = f'''
    tell application "Notes"
        make new note at folder "{folder}" with properties {{name:"{title}", body:"{content}"}}
        return "成功建立 '{title}'"
    end tell
    '''
    print(run_applescript(script))

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Apple Notes Agent CLI")
    subparsers = parser.add_subparsers(dest="command")

    # List folders
    subparsers.add_parser("folders", help="列出所有資料夾")

    # Search
    search_parser = subparsers.add_parser("search", help="搜尋關鍵字")
    search_parser.add_argument("keyword", help="關鍵字")

    # Get content
    get_parser = subparsers.add_parser("get", help="取得筆記內文")
    get_parser.add_argument("title", help="筆記標題")

    # Create note
    create_parser = subparsers.add_parser("create", help="新增筆記")
    create_parser.add_argument("title", help="標題")
    create_parser.add_argument("content", help="內容")
    create_parser.add_argument("--folder", default="Notes", help="指定資料夾")

    args = parser.parse_args()

    if args.command == "folders":
        list_folders()
    elif args.command == "search":
        search_notes(args.keyword)
    elif args.command == "get":
        get_note(args.title)
    elif args.command == "create":
        create_note(args.title, args.content, args.folder)
    else:
        parser.print_help()
