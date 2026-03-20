"""
ui_gtk/workers.py — Background threading helpers for GTK frontend.
Uses GLib.idle_add to post results back to the main thread.
"""
import threading
from gi.repository import GLib


def run_async(func, callback, *args, **kwargs):
    """Run func(*args, **kwargs) in a thread; call callback(result) on GTK main thread."""
    def _thread():
        try:
            result = func(*args, **kwargs)
        except Exception as e:
            result = None
            GLib.idle_add(callback, result, str(e))
            return
        GLib.idle_add(callback, result, None)

    threading.Thread(target=_thread, daemon=True).start()


def run_async_stream(func, on_line, on_done, *args, **kwargs):
    """Run func that yields lines; call on_line(line) per line, on_done(rc) when finished."""
    def _thread():
        try:
            for line in func(*args, **kwargs):
                GLib.idle_add(on_line, line)
            GLib.idle_add(on_done, 0, None)
        except Exception as e:
            GLib.idle_add(on_done, 1, str(e))

    threading.Thread(target=_thread, daemon=True).start()
