// CyoaEngine.cs — C# wrapper for the cyoa-native C-ABI.
//
// Usage:
//   // Load a story from a compiled .cyoa.bc file
//   byte[] bytecode = File.ReadAllBytes("forest_adventure.cyoa.bc");
//   var engine = new CyoaEngine(bytecode);
//
//   // Read the current event
//   string text = engine.CurrentEventText;
//   string[] choices = engine.CurrentChoices;
//
//   // Make a choice
//   engine.MakeChoice(0);
//
//   // Save / load
//   string json = engine.GetStateJson();
//   engine.SetStateJson(json);
//
//   // Clean up
//   engine.Dispose();
//
// Requires: Unity 2022.2+ (or .NET 5+ for System.Text.Json).
// For older Unity, add the Newtonsoft.Json NuGet package and
// replace the System.Text.Json calls with Newtonsoft.Json.

#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

#if SYSTEM_TEXT_JSON
using System.Text.Json;
using System.Text.Json.Serialization;
#endif

namespace Cyoa
{
    // ── Native function imports ──────────────────────────────────────────────

    internal static class Native
    {
        private const string DLL_NAME =
#if UNITY_EDITOR || UNITY_STANDALONE
            "cyoa_native";
#else
            "cyoa_native";
#endif

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

        [DllImport(DLL_NAME)]
        public static extern int cyoa_is_story_complete(IntPtr engine);

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

    // ── Helper: UTF-8 string marshalling ────────────────────────────────────

    internal static class StringMarshal
    {
        /// <summary>
        /// Read a NUL-terminated UTF-8 string from an unmanaged pointer.
        /// Returns null if the pointer is zero.
        /// </summary>
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

        /// <summary>
        /// Read a NUL-terminated UTF-8 string from an unmanaged pointer,
        /// then free the pointer with cyoa_free_string.
        /// Returns null if the pointer is zero.
        /// </summary>
        public static string? ReadAndFree(IntPtr ptr)
        {
            string? result = PtrToUtf8String(ptr);
            Native.cyoa_free_string(ptr);
            return result;
        }
    }

    // ── Data types ──────────────────────────────────────────────────────────

    /// <summary>
    /// Metadata for a registered story (returned by the story catalog).
    /// </summary>
    public struct StoryInfo
    {
        public string Name;
        public string[] Tags;
    }

    /// <summary>
    /// One entry in the choice history.
    /// </summary>
    public struct ChoiceHistoryEntry
    {
        public string EventId;
        public int ChoiceIndex;
        public string ChoiceText;
    }

    // ── Story catalog ───────────────────────────────────────────────────────

    /// <summary>
    /// Registry of compiled stories with tag-based filtering.
    /// Each registered story becomes an independent <see cref="CyoaEngine"/>.
    /// </summary>
    public class CyoaStoryCatalog : IDisposable
    {
        private IntPtr handle;
        private bool disposed;

        /// <summary>
        /// Create an empty story catalog.
        /// </summary>
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
            string tagsJson = JsonSerializer.Serialize(tags);
            IntPtr ptr = Native.cyoa_catalog_stories_with_all_tags_json(handle, tagsJson);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStoryArray(json);
        }

        /// <summary>
        /// Find all stories that have ANY of the specified tags.
        /// </summary>
        public StoryInfo[] StoriesWithAnyTags(string[] tags)
        {
            string tagsJson = JsonSerializer.Serialize(tags);
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

        /// <summary>
        /// Parse a JSON array of story objects into StoryInfo structs.
        /// Format: [{"name":"StoryName","tags":["tag1","tag2"]}, ...]
        /// </summary>
        private static StoryInfo[] ParseStoryArray(string? json)
        {
            if (string.IsNullOrEmpty(json))
                return new StoryInfo[0];

            // Simple JSON parser that doesn't depend on System.Text.Json
            return ParseStoryArrayManual(json);
        }

        private static StoryInfo[] ParseStoryArrayManual(string json)
        {
            json = json.Trim();
            if (json == "[]" || json.Length < 2)
                return new StoryInfo[0];

            var stories = new List<StoryInfo>();
            int pos = SkipWhitespace(json, 1); // skip opening '['

            while (pos < json.Length && json[pos] != ']')
            {
                // Parse one object: {"name":"...","tags":[...]}
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
                    // Read key
                    pos = SkipWhitespace(json, pos);
                    if (json[pos] != '"') break;
                    pos++;

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
            pos++; // skip opening '"'
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
            pos++; // skip '['
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
                pos++; // skip ']'
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

    // ── Engine wrapper ──────────────────────────────────────────────────────

    /// <summary>
    /// C# wrapper around a native CYOA engine instance.
    /// One engine per story — state is never shared between instances.
    /// </summary>
    public class CyoaEngine : IDisposable
    {
        internal IntPtr handle;
        private bool disposed;

        /// <summary>
        /// Create an engine directly from compiled bytecode bytes.
        /// </summary>
        public CyoaEngine(byte[] bytecode)
        {
            handle = Native.cyoa_create(bytecode, (UIntPtr)bytecode.Length);
            if (handle == IntPtr.Zero)
                throw new Exception("Failed to create CYOA engine (check bytecode)");
        }

        /// <summary>
        /// Wrap a native engine handle created by the catalog.
        /// </summary>
        internal CyoaEngine(IntPtr existingHandle)
        {
            handle = existingHandle;
            if (handle == IntPtr.Zero)
                throw new Exception("Invalid engine handle");
        }

        // ── Event queries ───────────────────────────────────────────────────

        /// <summary>
        /// Current event's internal ID (name).
        /// </summary>
        public string CurrentEventId =>
            StringMarshal.PtrToUtf8String(Native.cyoa_current_event_id(handle)) ?? "";

        /// <summary>
        /// Current event text — paragraphs joined by newlines.
        /// </summary>
        public string CurrentEventText =>
            StringMarshal.PtrToUtf8String(Native.cyoa_current_event_text(handle)) ?? "";

        /// <summary>
        /// Number of currently available choices.
        /// </summary>
        public int ChoiceCount => Native.cyoa_current_choice_count(handle);

        /// <summary>
        /// Get the text of a choice at the given index.
        /// Returns null if index is out of bounds.
        /// </summary>
        public string? GetChoiceText(int index) =>
            StringMarshal.PtrToUtf8String(Native.cyoa_choice_text(handle, index));

        /// <summary>
        /// Get all current choices as an array.
        /// </summary>
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

        /// <summary>
        /// Apply the player's choice at the given index.
        /// After this call, the engine has advanced to the next event.
        /// </summary>
        public void MakeChoice(int index)
        {
            Native.cyoa_make_choice(handle, index);
        }

        /// <summary>
        /// Effect text from the most recent <see cref="MakeChoice"/> call.
        /// Multiple effect texts are joined by newlines.
        /// </summary>
        public string LastEffectText =>
            StringMarshal.PtrToUtf8String(Native.cyoa_last_effect_text(handle)) ?? "";

        // ── Choice history ──────────────────────────────────────────────────

        /// <summary>
        /// Number of entries in the choice history.
        /// </summary>
        public int HistoryLength => Native.cyoa_history_length(handle);

        /// <summary>
        /// Get a history entry at the given index.
        /// Returns null if the index is out of bounds.
        /// </summary>
        public ChoiceHistoryEntry? GetHistoryEntry(int index)
        {
            IntPtr ptr = Native.cyoa_history_entry(handle, index);
            string? json = StringMarshal.PtrToUtf8String(ptr);
            if (json == null) return null;
            return ParseHistoryEntry(json);
        }

        /// <summary>
        /// Get all choice history entries.
        /// </summary>
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

            // Parse {"eventId":"...","choiceIndex":N,"choiceText":"..."}
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
            string key = $"\"{field}\":";
            int idx = json.IndexOf(key, StringComparison.Ordinal);
            if (idx < 0) return false;

            int valStart = idx + key.Length;
            valStart = SkipWhitespaceLocal(json, valStart);

            if (valStart >= json.Length) return false;

            if (json[valStart] == '"')
            {
                // String value
                int end = json.IndexOf('"', valStart + 1);
                if (end < 0) return false;
                value = json.Substring(valStart + 1, end - valStart - 1);
                return true;
            }
            else
            {
                // Non-string value
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
            string key = $"\"{field}\":";
            int idx = json.IndexOf(key, StringComparison.Ordinal);
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

        /// <summary>
        /// Serialize the full player state as a JSON string.
        /// Use this for save files.
        /// </summary>
        public string GetStateJson()
        {
            IntPtr ptr = Native.cyoa_get_state_json(handle);
            return StringMarshal.ReadAndFree(ptr) ?? "";
        }

        /// <summary>
        /// Restore player state from a JSON string produced by
        /// <see cref="GetStateJson"/>.
        /// </summary>
        public void SetStateJson(string json)
        {
            Native.cyoa_set_state_json(handle, json);
        }

        // ── Queries ─────────────────────────────────────────────────────────

        /// <summary>
        /// All event IDs in the story as an array.
        /// </summary>
        public string[] GetAvailableEvents()
        {
            IntPtr ptr = Native.cyoa_available_events_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        /// <summary>
        /// Check whether an event by ID is reachable.
        /// </summary>
        public bool CanAccessEvent(string id) =>
            Native.cyoa_can_access_event(handle, id) != 0;

        /// <summary>
        /// Returns true if the story has ended (a terminal choice was made).
        /// A terminal choice is one that has no `next` event specified.
        /// </summary>
        public bool IsStoryComplete =>
            Native.cyoa_is_story_complete(handle) != 0;

        // ── Stats / tags / flags ────────────────────────────────────────────

        /// <summary>
        /// Get all stats as a dictionary of name → value.
        /// </summary>
        public Dictionary<string, int> GetStats()
        {
            IntPtr ptr = Native.cyoa_list_stats_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStats(json);
        }

        /// <summary>
        /// Get a stat value by name. Returns 0 if the stat doesn't exist.
        /// </summary>
        public int GetStat(string name) =>
            Native.cyoa_get_stat(handle, name);

        /// <summary>
        /// Story-level tags (static metadata declared in the story).
        /// </summary>
        public string[] GetStoryTags()
        {
            IntPtr ptr = Native.cyoa_list_story_tags_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        /// <summary>
        /// Runtime tags currently applied during play.
        /// </summary>
        public string[] GetTags()
        {
            IntPtr ptr = Native.cyoa_list_tags_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        /// <summary>
        /// Runtime flags currently set.
        /// </summary>
        public string[] GetFlags()
        {
            IntPtr ptr = Native.cyoa_list_flags_json(handle);
            string? json = StringMarshal.ReadAndFree(ptr);
            return ParseStringArray(json);
        }

        // ── JSON parsing helpers (no external dependency) ───────────────────

        private static string[] ParseStringArray(string? json)
        {
            if (string.IsNullOrEmpty(json) || json.Trim() == "[]")
                return new string[0];

            var list = new List<string>();
            var trimmed = json.Trim();
            if (!trimmed.StartsWith("[")) return new string[0];

            int pos = 1; // skip '['
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

            int pos = 1; // skip '{'
            while (pos < trimmed.Length && trimmed[pos] != '}')
            {
                pos = SkipWhitespaceLocal(trimmed, pos);
                if (pos >= trimmed.Length || trimmed[pos] != '"') break;

                // Read key
                pos++; // skip '"'
                int keyStart = pos;
                while (pos < trimmed.Length && trimmed[pos] != '"') pos++;
                string key = trimmed.Substring(keyStart, pos - keyStart);
                pos++; // skip '"'

                pos = SkipWhitespaceLocal(trimmed, pos);
                if (pos >= trimmed.Length || trimmed[pos] != ':') break;
                pos++; // skip ':'
                pos = SkipWhitespaceLocal(trimmed, pos);

                // Read value (integer)
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
