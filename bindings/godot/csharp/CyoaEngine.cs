// CyoaEngine.cs — C# wrapper for the CYOA engine, targeting Godot 4.x (.NET).
//
// Unlike Unity, Godot supports both C# (via Godot.NET) and GDScript.
// GDScript cannot call native C functions directly, so this C# wrapper
// serves as the bridge: it loads the native C-ABI library and exposes
// a Godot-friendly API that GDScript can call via `CyoaEngineGD` in
// cyoa_engine.gd.
//
// Usage (C#):
//   var engine = CyoaEngine.LoadFromFile("res://data/forest_adventure.cyoa.bc");
//   GD.Print(engine.CurrentEventText);
//   string[] choices = engine.CurrentChoices;
//
// Usage (GDScript via cyoa_engine.gd):
//   var engine = CyoaEngineGD.new()
//   engine.load_from_path("res://data/forest_adventure.cyoa.bc")
//   print(engine.get_event_text())
//   engine.make_choice(0)
//
// Requires: Godot 4.x with .NET support, cyoa_native shared library.

#nullable enable

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

using Godot;

namespace Cyoa.Godot
{
    // ── Native function imports ────────────────────────────────────────────────

    internal static class Native
    {
        private const string DLL_NAME = "cyoa_native";

        // Lifecycle
        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_create(byte[] bytecode, UIntPtr len);

        [DllImport(DLL_NAME)]
        public static extern void cyoa_destroy(IntPtr engine);

        // Event queries
        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_current_event_id(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_current_event_text(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern int cyoa_current_choice_count(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_choice_text(IntPtr engine, int index);

        // Make a choice
        [DllImport(DLL_NAME)]
        public static extern void cyoa_make_choice(IntPtr engine, int index);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_last_effect_text(IntPtr engine);

        // Choice history
        [DllImport(DLL_NAME)]
        public static extern int cyoa_history_length(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_history_entry(IntPtr engine, int index);

        // State management
        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_get_state_json(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern void cyoa_set_state_json(IntPtr engine, string json);

        [DllImport(DLL_NAME)]
        public static extern void cyoa_free_string(IntPtr s);

        // Queries
        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_available_events_json(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern int cyoa_can_access_event(IntPtr engine, string id);

        // Stats / tags / flags
        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_list_stats_json(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_list_story_tags_json(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_list_tags_json(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_list_flags_json(IntPtr engine);

        [DllImport(DLL_NAME)]
        public static extern int cyoa_get_stat(IntPtr engine, string name);

        // Catalog (multi-story)
        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_create();

        [DllImport(DLL_NAME)]
        public static extern void cyoa_catalog_destroy(IntPtr catalog);

        [DllImport(DLL_NAME)]
        public static extern int cyoa_catalog_register(IntPtr catalog, byte[] bytecode, UIntPtr len);

        [DllImport(DLL_NAME)]
        public static extern int cyoa_catalog_story_count(IntPtr catalog);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_list_stories_json(IntPtr catalog);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_stories_with_tag_json(IntPtr catalog, string tag);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_stories_with_all_tags_json(IntPtr catalog, string tags_json);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_stories_with_any_tags_json(IntPtr catalog, string tags_json);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_create_engine(IntPtr catalog, int index);

        [DllImport(DLL_NAME)]
        public static extern IntPtr cyoa_catalog_create_engine_by_name(IntPtr catalog, string name);
    }

    // ── Helper: UTF-8 string marshalling ───────────────────────────────────────

    internal static class StringMarshal
    {
        public static string? PtrToUtf8String(IntPtr ptr)
        {
            if (ptr == IntPtr.Zero)
                return null;

            // Find the length by scanning for NUL
            int len = 0;
            while (Marshal.ReadByte(ptr, len) != 0)
                len++;

            if (len == 0)
                return string.Empty;

            byte[] bytes = new byte[len];
            Marshal.Copy(ptr, bytes, 0, len);
            return Encoding.UTF8.GetString(bytes);
        }

        public static string? ReadAndFree(IntPtr ptr)
        {
            string? result = PtrToUtf8String(ptr);
            Native.cyoa_free_string(ptr);
            return result;
        }
    }

    // ── Data types ─────────────────────────────────────────────────────────────

    public struct StoryInfo
    {
        public string Name;
        public string[] Tags;
    }

    public struct ChoiceHistoryEntry
    {
        public string EventId;
        public int ChoiceIndex;
        public string ChoiceText;
    }

    // ── Story catalog ─────────────────────────────────────────────────────────

    /// <summary>
    /// Registry of compiled stories with tag-based filtering.
    /// Use this to discover stories before creating a <see cref="CyoaEngine"/>.
    /// </summary>
    public class CyoaStoryCatalog : IDisposable
    {
        private IntPtr handle;
        private bool disposed;

        public CyoaStoryCatalog()
        {
            handle = Native.cyoa_catalog_create();
            if (handle == IntPtr.Zero)
                throw new Exception("Failed to create CYOA story catalog");
        }

        /// <summary>
        /// Number of registered stories.
        /// </summary>
        public int StoryCount => Native.cyoa_catalog_story_count(handle);

        /// <summary>
        /// Register a compiled story from bytecode bytes.
        /// Returns true on success.
        /// </summary>
        public bool RegisterStory(byte[] bytecode)
        {
            int result = Native.cyoa_catalog_register(
                handle,
                bytecode,
                (UIntPtr)bytecode.Length);
            return result != 0;
        }

        /// <summary>
        /// Register a compiled story loaded from a Godot resource path.
        /// </summary>
        public bool RegisterStoryFromFile(string path)
        {
            byte[] bytes = FileAccess.GetFileAsBytes(path);
            return RegisterStory(bytes);
        }

        /// <summary>
        /// List all registered stories.
        /// </summary>
        public StoryInfo[] ListStories()
        {
            IntPtr ptr = Native.cyoa_catalog_list_stories_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStoryArray(json);
        }

        /// <summary>
        /// Find all stories that have a specific tag.
        /// </summary>
        public StoryInfo[] StoriesWithTag(string tag)
        {
            IntPtr ptr = Native.cyoa_catalog_stories_with_tag_json(handle, tag);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStoryArray(json);
        }

        /// <summary>
        /// Find all stories that have ALL of the specified tags.
        /// </summary>
        public StoryInfo[] StoriesWithAllTags(string[] tags)
        {
            string tagsJson = "[" + string.Join(",", Array.ConvertAll(tags, t => "\"" + t + "\"")) + "]";
            IntPtr ptr = Native.cyoa_catalog_stories_with_all_tags_json(handle, tagsJson);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStoryArray(json);
        }

        /// <summary>
        /// Find all stories that have ANY of the specified tags.
        /// </summary>
        public StoryInfo[] StoriesWithAnyTags(string[] tags)
        {
            string tagsJson = "[" + string.Join(",", Array.ConvertAll(tags, t => "\"" + t + "\"")) + "]";
            IntPtr ptr = Native.cyoa_catalog_stories_with_any_tags_json(handle, tagsJson);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStoryArray(json);
        }

        /// <summary>
        /// Create an engine for the story at the given index.
        /// </summary>
        public CyoaEngine CreateEngine(int index)
        {
            IntPtr enginePtr = Native.cyoa_catalog_create_engine(handle, index);
            if (enginePtr == IntPtr.Zero)
                throw new Exception($"Failed to create engine at index {index}");
            return new CyoaEngine(enginePtr);
        }

        /// <summary>
        /// Find a story by name and create an engine for it.
        /// Throws if no story with that name is registered.
        /// </summary>
        public CyoaEngine CreateEngineByName(string name)
        {
            IntPtr enginePtr = Native.cyoa_catalog_create_engine_by_name(handle, name);
            if (enginePtr == IntPtr.Zero)
                throw new Exception($"Story '{name}' not found in catalog");
            return new CyoaEngine(enginePtr);
        }

        // ── JSON parsing helpers (no external dependency) ─────────────────────

        private static StoryInfo[] ParseStoryArray(string? json)
        {
            if (string.IsNullOrEmpty(json))
                return new StoryInfo[0];

            json = json.Trim();
            if (json == "[]" || json.Length < 2)
                return new StoryInfo[0];

            var stories = new List<StoryInfo>();
            int pos = SkipWhitespace(json, 1); // skip opening '['

            while (pos < json.Length && json[pos] != ']')
            {
                if (json[pos] != '{')
                {
                    pos = SkipValue(json, pos);
                    continue;
                }

                pos++; // skip '{'
                pos = SkipWhitespace(json, pos);

                string name = "";
                string[] tags = new string[0];

                while (pos < json.Length && json[pos] != '}')
                {
                    pos = SkipWhitespace(json, pos);
                    if (pos >= json.Length || json[pos] != '"') break;
                    pos++; // skip key opening '"'

                    int keyStart = pos;
                    while (pos < json.Length && json[pos] != '"') pos++;
                    string key = json.Substring(keyStart, pos - keyStart);
                    pos++; // skip closing '"'

                    pos = SkipWhitespace(json, pos);
                    if (pos >= json.Length || json[pos] != ':') break;
                    pos++; // skip ':'
                    pos = SkipWhitespace(json, pos);

                    if (key == "name")
                    {
                        name = ReadQuotedString(json, ref pos);
                    }
                    else if (key == "tags")
                    {
                        tags = ReadStringArray(json, ref pos);
                    }
                    else
                    {
                        pos = SkipValue(json, pos);
                    }

                    pos = SkipWhitespace(json, pos);
                    if (pos < json.Length && json[pos] == ',')
                        pos++;
                }

                pos = SkipWhitespace(json, pos);
                if (pos < json.Length && json[pos] == '}')
                    pos++;

                stories.Add(new StoryInfo { Name = name, Tags = tags });

                pos = SkipWhitespace(json, pos);
                if (pos < json.Length && json[pos] == ',')
                    pos++;
            }

            return stories.ToArray();
        }

        private static int SkipWhitespace(string s, int pos)
        {
            while (pos < s.Length && char.IsWhiteSpace(s[pos]))
                pos++;
            return pos;
        }

        private static string ReadQuotedString(string s, ref int pos)
        {
            if (pos >= s.Length || s[pos] != '"')
                return "";
            pos++;
            var sb = new StringBuilder();
            while (pos < s.Length && s[pos] != '"')
            {
                if (s[pos] == '\\' && pos + 1 < s.Length)
                {
                    sb.Append(s[pos + 1]);
                    pos += 2;
                }
                else
                {
                    sb.Append(s[pos]);
                    pos++;
                }
            }
            pos++; // skip closing '"'
            return sb.ToString();
        }

        private static string[] ReadStringArray(string s, ref int pos)
        {
            if (pos >= s.Length || s[pos] != '[')
                return new string[0];
            pos++;
            var list = new List<string>();
            while (pos < s.Length && s[pos] != ']')
            {
                pos = SkipWhitespace(s, pos);
                list.Add(ReadQuotedString(s, ref pos));
                pos = SkipWhitespace(s, pos);
                if (pos < s.Length && s[pos] == ',')
                    pos++;
            }
            if (pos < s.Length && s[pos] == ']')
                pos++;
            return list.ToArray();
        }

        private static int SkipValue(string s, int pos)
        {
            pos = SkipWhitespace(s, pos);
            if (pos >= s.Length) return pos;

            if (s[pos] == '"')
            {
                pos++;
                while (pos < s.Length && s[pos] != '"')
                {
                    if (s[pos] == '\\') pos += 2;
                    else pos++;
                }
                pos++;
            }
            else if (s[pos] == '{')
            {
                int depth = 1;
                pos++;
                while (pos < s.Length && depth > 0)
                {
                    if (s[pos] == '{') depth++;
                    if (s[pos] == '}') depth--;
                    pos++;
                }
            }
            else if (s[pos] == '[')
            {
                int depth = 1;
                pos++;
                while (pos < s.Length && depth > 0)
                {
                    if (s[pos] == '[') depth++;
                    if (s[pos] == ']') depth--;
                    pos++;
                }
            }
            else
            {
                while (pos < s.Length && !char.IsWhiteSpace(s[pos]) && s[pos] != ',' && s[pos] != ']' && s[pos] != '}')
                    pos++;
            }

            return pos;
        }

        // ── IDisposable ───────────────────────────────────────────────────────

        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing)
        {
            if (!disposed)
            {
                if (handle != IntPtr.Zero)
                {
                    Native.cyoa_catalog_destroy(handle);
                    handle = IntPtr.Zero;
                }
                disposed = true;
            }
        }

        ~CyoaStoryCatalog() => Dispose(false);
    }

    // ── Engine wrapper ────────────────────────────────────────────────────────

    /// <summary>
    /// C# wrapper around a native CYOA engine instance.
    /// One engine per story — state is never shared between instances.
    /// </summary>
    public class CyoaEngine : IDisposable
    {
        internal IntPtr handle;
        private bool disposed;

        public CyoaEngine(byte[] bytecode)
        {
            handle = Native.cyoa_create(bytecode, (UIntPtr)bytecode.Length);
            if (handle == IntPtr.Zero)
                throw new Exception("Failed to create CYOA engine (check bytecode)");
        }

        internal CyoaEngine(IntPtr existingHandle)
        {
            handle = existingHandle;
            if (handle == IntPtr.Zero)
                throw new Exception("Invalid engine handle");
        }

        /// <summary>
        /// Load a story from a Godot resource path (.cyoa.bc file).
        /// </summary>
        public static CyoaEngine LoadFromFile(string path)
        {
            byte[] bytes = FileAccess.GetFileAsBytes(path);
            if (bytes.Length == 0)
                throw new Exception($"Could not read bytecode from: {path}");
            return new CyoaEngine(bytes);
        }

        // ── Event queries ───────────────────────────────────────────────────

        public string CurrentEventId =>
            StringMarshal.PtrToUtf8String(Native.cyoa_current_event_id(handle)) ?? "";

        public string CurrentEventText =>
            StringMarshal.PtrToUtf8String(Native.cyoa_current_event_text(handle)) ?? "";

        public int ChoiceCount => Native.cyoa_current_choice_count(handle);

        public string? GetChoiceText(int index) =>
            StringMarshal.PtrToUtf8String(Native.cyoa_choice_text(handle, index));

        public string[] CurrentChoices
        {
            get
            {
                int count = ChoiceCount;
                var choices = new string[count];
                for (int i = 0; i < count; i++)
                {
                    choices[i] = GetChoiceText(i) ?? "";
                }
                return choices;
            }
        }

        // ── Make a choice ───────────────────────────────────────────────────

        public void MakeChoice(int index)
        {
            Native.cyoa_make_choice(handle, index);
        }

        public string LastEffectText =>
            StringMarshal.PtrToUtf8String(Native.cyoa_last_effect_text(handle)) ?? "";

        // ── Choice history ──────────────────────────────────────────────────

        public int HistoryLength => Native.cyoa_history_length(handle);

        public ChoiceHistoryEntry? GetHistoryEntry(int index)
        {
            IntPtr ptr = Native.cyoa_history_entry(handle, index);
            string? json = StringMarshal.PtrToUtf8String(ptr);
            if (json == null) return null;
            return ParseHistoryEntry(json);
        }

        public ChoiceHistoryEntry[] GetAllHistory()
        {
            int len = HistoryLength;
            var entries = new ChoiceHistoryEntry[len];
            for (int i = 0; i < len; i++)
            {
                entries[i] = GetHistoryEntry(i) ?? new ChoiceHistoryEntry();
            }
            return entries;
        }

        private static ChoiceHistoryEntry ParseHistoryEntry(string json)
        {
            var entry = new ChoiceHistoryEntry();

            if (TryExtractJsonField(json, "eventId", out string? val))
                entry.EventId = val ?? "";
            if (TryExtractJsonField(json, "choiceText", out val))
                entry.ChoiceText = val ?? "";
            if (TryExtractJsonInt(json, "choiceIndex", out int idx))
                entry.ChoiceIndex = idx;

            return entry;
        }

        private static bool TryExtractJsonField(string json, string field, out string? value)
        {
            value = null;
            string key = "\"" + field + "\":";
            int idx = json.IndexOf(key);
            if (idx < 0) return false;

            int valStart = idx + key.Length;
            valStart = SkipWhitespaceLocal(json, valStart);

            if (valStart >= json.Length) return false;

            if (json[valStart] == '"')
            {
                int end = json.IndexOf('"', valStart + 1);
                if (end < 0) return false;
                value = json.Substring(valStart + 1, end - valStart - 1);
                return true;
            }
            else
            {
                int end = valStart;
                while (end < json.Length && !char.IsWhiteSpace(json[end]) && json[end] != ',' && json[end] != '}')
                    end++;
                value = json.Substring(valStart, end - valStart);
                return true;
            }
        }

        private static bool TryExtractJsonInt(string json, string field, out int value)
        {
            value = 0;
            string key = "\"" + field + "\":";
            int idx = json.IndexOf(key);
            if (idx < 0) return false;

            int valStart = idx + key.Length;
            valStart = SkipWhitespaceLocal(json, valStart);

            if (valStart >= json.Length) return false;

            int end = valStart;
            while (end < json.Length && char.IsDigit(json[end]) || (end == valStart && json[end] == '-'))
                end++;

            return int.TryParse(json.Substring(valStart, end - valStart), out value);
        }

        private static int SkipWhitespaceLocal(string s, int pos)
        {
            while (pos < s.Length && char.IsWhiteSpace(s[pos]))
                pos++;
            return pos;
        }

        // ── State management ────────────────────────────────────────────────

        public string GetStateJson()
        {
            IntPtr ptr = Native.cyoa_get_state_json(handle);
            return StringMarshal.ReadAndFree(ptr) ?? "";
        }

        public void SetStateJson(string json)
        {
            Native.cyoa_set_state_json(handle, json);
        }

        // ── Queries ─────────────────────────────────────────────────────────

        public string[] GetAvailableEvents()
        {
            IntPtr ptr = Native.cyoa_available_events_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        public bool CanAccessEvent(string id) =>
            Native.cyoa_can_access_event(handle, id) != 0;

        public bool IsStoryComplete =>
            Native.cyoa_is_story_complete(handle) != 0;

        // ── Stats / tags / flags ────────────────────────────────────────────

        public Dictionary<string, int> GetStats()
        {
            IntPtr ptr = Native.cyoa_list_stats_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStats(json);
        }

        public int GetStat(string name) =>
            Native.cyoa_get_stat(handle, name);

        public string[] GetStoryTags()
        {
            IntPtr ptr = Native.cyoa_list_story_tags_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        public string[] GetTags()
        {
            IntPtr ptr = Native.cyoa_list_tags_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        public string[] GetFlags()
        {
            IntPtr ptr = Native.cyoa_list_flags_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        // ── JSON parsing helpers ────────────────────────────────────────────

        private static string[] ParseStringArray(string? json)
        {
            if (string.IsNullOrEmpty(json) || json.Trim() == "[]")
                return new string[0];

            var list = new List<string>();
            var trimmed = json.Trim();
            if (!trimmed.StartsWith("[")) return new string[0];

            int pos = 1;
            var sb = new StringBuilder();
            bool inString = false;

            while (pos < trimmed.Length)
            {
                char c = trimmed[pos];

                if (c == '"')
                {
                    inString = !inString;
                    if (!inString && sb.Length > 0)
                    {
                        list.Add(sb.ToString());
                        sb.Clear();
                    }
                    pos++;
                }
                else if (inString)
                {
                    if (c == '\\' && pos + 1 < trimmed.Length)
                    {
                        sb.Append(trimmed[pos + 1]);
                        pos += 2;
                    }
                    else
                    {
                        sb.Append(c);
                        pos++;
                    }
                }
                else
                {
                    pos = SkipWhitespaceLocal(trimmed, pos);
                    if (pos < trimmed.Length && trimmed[pos] == ',')
                    {
                        pos++;
                        pos = SkipWhitespaceLocal(trimmed, pos);
                    }
                    else if (pos < trimmed.Length && trimmed[pos] == ']')
                    {
                        break;
                    }
                }
            }

            return list.ToArray();
        }

        private static Dictionary<string, int> ParseStats(string? json)
        {
            var stats = new Dictionary<string, int>();
            if (string.IsNullOrEmpty(json)) return stats;

            var trimmed = json.Trim();
            if (!trimmed.StartsWith("{")) return stats;

            int pos = 1;
            while (pos < trimmed.Length && trimmed[pos] != '}')
            {
                pos = SkipWhitespaceLocal(trimmed, pos);
                if (pos >= trimmed.Length || trimmed[pos] != '"') break;

                pos++;
                int keyStart = pos;
                while (pos < trimmed.Length && trimmed[pos] != '"') pos++;
                string key = trimmed.Substring(keyStart, pos - keyStart);
                pos++;

                pos = SkipWhitespaceLocal(trimmed, pos);
                if (pos >= trimmed.Length || trimmed[pos] != ':') break;
                pos++;
                pos = SkipWhitespaceLocal(trimmed, pos);

                int valStart = pos;
                while (pos < trimmed.Length && char.IsDigit(trimmed[pos]) || (pos == valStart && trimmed[pos] == '-'))
                    pos++;
                if (int.TryParse(trimmed.Substring(valStart, pos - valStart), out int val))
                    stats[key] = val;

                pos = SkipWhitespaceLocal(trimmed, pos);
                if (pos < trimmed.Length && trimmed[pos] == ',') pos++;
            }

            return stats;
        }

        // ── IDisposable ───────────────────────────────────────────────────────

        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing)
        {
            if (!disposed)
            {
                if (handle != IntPtr.Zero)
                {
                    Native.cyoa_destroy(handle);
                    handle = IntPtr.Zero;
                }
                disposed = true;
            }
        }

        ~CyoaEngine() => Dispose(false);
    }
}
