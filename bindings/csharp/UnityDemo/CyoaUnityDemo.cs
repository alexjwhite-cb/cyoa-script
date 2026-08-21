// CyoaUnityDemo.cs — Unity demo showcasing the CYOA engine C# wrapper.
//
// This MonoBehaviour loads two compiled stories (main quest + side quest)
// simultaneously using CyoaStoryCatalog, and presents a simple UI for
// reading the event narrative and choosing options.
//
// ## Scene Setup
//
// Create a Unity scene with the following UI hierarchy:
//
//   Canvas (Screen Space - Overlay)
//   ├── StoryCatalogPanel (GameObject)
//   │   ├── TitleText        (TextMeshPro - Text)
//   │   ├── TagInputField    (TMP_InputField)
//   │   ├── FilterButtons    (Horizontal Layout Group)
//   │   │   ├── BtnAllTags   (Button - "Require All Tags")
//   │   │   └── BtnAnyTags   (Button - "Any Tag Match")
//   │   └── StoryList        (Vertical Layout Group)
//   │       └── StoryCardPrefab (prefab with Button + Text)
//   ├── StoryPanel (GameObject)
//   │   ├── BackButton       (Button)
//   │   ├── EventIdText      (TextMeshPro - Text)
//   │   ├── ProseText        (TextMeshPro - Text)
//   │   ├── ChoicesPanel     (Vertical Layout Group)
//   │   │   └── ChoiceButtonPrefab (prefab with Button + Text)
//   │   └── StatsText        (TextMeshPro - Text)
//   └── SaveLoadPanel (GameObject)
//       ├── SaveButton       (Button)
//       ├── LoadButton       (Button)
//       └── SaveTextArea     (TMP_InputField - read only)
//
// Assign all references in the Inspector. The demo bytecode files
// (forest_adventure.cyoa.bc and tavern_tales.cyoa.bc) are expected
// in the StreamingAssets folder.
//
// Requires: TextMeshPro package, cyoa_native native plugin.

#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEngine;
using UnityEngine.UI;

#if TMP_AVAILABLE
using TMPro;
#endif

namespace Cyoa.Unity
{
    /// <summary>
    /// Unity demo that loads two stories simultaneously and lets the player
    /// explore them with full choice history and save/load support.
    /// </summary>
    public class CyoaUnityDemo : MonoBehaviour
    {
        // ── UI references (assign in Inspector) ────────────────────────────

        [Header("Catalog UI")]
        public GameObject? catalogPanel;
        public TMP_Text? titleText;
        public TMP_InputField? tagInputField;
        public Button? btnAllTags;
        public Button? btnAnyTags;
        public Transform? storyListContainer;
        public GameObject? storyCardPrefab;

        [Header("Story UI")]
        public GameObject? storyPanel;
        public Button? backButton;
        public TMP_Text? eventIdText;
        public TMP_Text? proseText;
        public Transform? choicesContainer;
        public GameObject? choiceButtonPrefab;
        public TMP_Text? statsText;

        [Header("Save/Load")]
        public Button? saveButton;
        public Button? loadButton;
        public TMP_InputField? saveTextArea;

        // ── Internal state ──────────────────────────────────────────────────

        private Cyoa.Unity.CyoaStoryCatalog? catalog;
        private Cyoa.Unity.CyoaEngine? currentEngine;
        private string? lastSavedJson;

        // ── Unity lifecycle ─────────────────────────────────────────────────

        private void Start()
        {
            SetupButtons();
            LoadCatalog();
        }

        private void SetupButtons()
        {
            if (btnAllTags != null) btnAllTags.onClick.AddListener(() => FilterCatalog("all"));
            if (btnAnyTags != null) btnAnyTags.onClick.AddListener(() => FilterCatalog("any"));
            if (backButton != null) backButton.onClick.AddListener(BackToCatalog);
            if (saveButton != null) saveButton.onClick.AddListener(SaveGame);
            if (loadButton != null) loadButton.onClick.AddListener(LoadGame);
        }

        // ── Data loading ────────────────────────────────────────────────────

        private void LoadCatalog()
        {
            catalog = new Cyoa.Unity.CyoaStoryCatalog();

            // Load compiled bytecode from StreamingAssets
            string mainQuestPath = Path.Combine(Application.streamingAssetsPath, "forest_adventure.cyoa.bc");
            string sideQuestPath = Path.Combine(Application.streamingAssetsPath, "tavern_tales.cyoa.bc");

            if (File.Exists(mainQuestPath))
            {
                byte[] bytes = File.ReadAllBytes(mainQuestPath);
                if (catalog.RegisterStory(bytes))
                {
                    Debug.Log($"Registered: ForestAdventure");
                }
            }

            if (File.Exists(sideQuestPath))
            {
                byte[] bytes = File.ReadAllBytes(sideQuestPath);
                if (catalog.RegisterStory(bytes))
                {
                    Debug.Log($"Registered: TavernTales");
                }
            }

            ShowCatalog();
        }

        // ── Catalog display ─────────────────────────────────────────────────

        private void ShowCatalog()
        {
            if (catalogPanel != null) catalogPanel.SetActive(true);
            if (storyPanel != null) storyPanel.SetActive(false);

            if (titleText != null)
                titleText.text = $"CYOA Story Catalog ({catalog?.StoryCount ?? 0} stories)";

            RefreshStoryList();
        }

        private void RefreshStoryList()
        {
            if (storyListContainer == null || storyCardPrefab == null) return;

            // Clear existing cards
            foreach (Transform child in storyListContainer)
                Destroy(child.gameObject);

            var stories = catalog?.ListStories() ?? new Cyoa.Unity.StoryInfo[0];

            foreach (var story in stories)
            {
                var cardObj = Instantiate(storyCardPrefab, storyListContainer);
                var cardText = cardObj.GetComponentInChildren<TMP_Text>();
                if (cardText != null)
                {
                    var sb = new StringBuilder();
                    sb.AppendLine(story.Name);
                    if (story.Tags != null && story.Tags.Length > 0)
                        sb.AppendLine("Tags: " + string.Join(", ", story.Tags));
                    cardText.text = sb.ToString();
                }

                var cardButton = cardObj.GetComponent<Button>();
                int storyIndex = Array.IndexOf(stories, story);
                if (cardButton != null)
                {
                    cardButton.onClick.AddListener(() => SelectStory(storyIndex));
                }
            }
        }

        private void FilterCatalog(string mode)
        {
            if (catalog == null || tagInputField == null) return;

            string tagStr = tagInputField.text.Trim();
            Cyoa.Unity.StoryInfo[] stories;

            if (string.IsNullOrEmpty(tagStr))
            {
                stories = catalog.ListStories();
            }
            else
            {
                try
                {
                    // Try parsing as JSON array first
                    string[] tags = tagStr.StartsWith("[")
                        ? ParseJsonStringArray(tagStr)
                        : new[] { tagStr };

                    if (mode == "all")
                        stories = catalog.StoriesWithAllTags(tags);
                    else
                        stories = catalog.StoriesWithAnyTags(tags);
                }
                catch
                {
                    // Fall back to single-tag matching
                    stories = catalog.StoriesWithTag(tagStr);
                }
            }

            RenderStoryCards(stories);
        }

        private void RenderStoryCards(Cyoa.Unity.StoryInfo[] stories)
        {
            if (storyListContainer == null || storyCardPrefab == null) return;

            foreach (Transform child in storyListContainer)
                Destroy(child.gameObject);

            for (int i = 0; i < stories.Length; i++)
            {
                var story = stories[i];
                var cardObj = Instantiate(storyCardPrefab, storyListContainer);
                var cardText = cardObj.GetComponentInChildren<TMP_Text>();
                if (cardText != null)
                {
                    var sb = new StringBuilder();
                    sb.AppendLine(story.Name);
                    if (story.Tags != null && story.Tags.Length > 0)
                        sb.AppendLine("Tags: " + string.Join(", ", story.Tags));
                    cardText.text = sb.ToString();
                }

                var cardButton = cardObj.GetComponent<Button>();
                int idx = i;
                if (cardButton != null)
                {
                    cardButton.onClick.AddListener(() => SelectStory(idx));
                }
            }
        }

        private string[] ParseJsonStringArray(string json)
        {
            var list = new List<string>();
            var trimmed = json.Trim();
            if (!trimmed.StartsWith("[")) return new string[0];

            int pos = 1;
            while (pos < trimmed.Length && trimmed[pos] != ']')
            {
                pos = SkipWhitespace(trimmed, pos);
                if (pos >= trimmed.Length || trimmed[pos] != '"') break;
                pos++;

                var sb = new StringBuilder();
                while (pos < trimmed.Length && trimmed[pos] != '"')
                {
                    if (trimmed[pos] == '\\' && pos + 1 < trimmed.Length)
                    {
                        sb.Append(trimmed[pos + 1]);
                        pos += 2;
                    }
                    else
                    {
                        sb.Append(trimmed[pos]);
                        pos++;
                    }
                }
                pos++; // skip closing "
                list.Add(sb.ToString());

                pos = SkipWhitespace(trimmed, pos);
                if (pos < trimmed.Length && trimmed[pos] == ',') pos++;
            }

            return list.ToArray();
        }

        private static int SkipWhitespace(string s, int pos)
        {
            while (pos < s.Length && char.IsWhiteSpace(s[pos]))
                pos++;
            return pos;
        }

        // ── Story play ──────────────────────────────────────────────────────

        private void SelectStory(int index)
        {
            if (catalog == null) return;

            currentEngine?.Dispose();
            currentEngine = catalog.CreateEngine(index);

            if (catalogPanel != null) catalogPanel.SetActive(false);
            if (storyPanel != null) storyPanel.SetActive(true);

            RenderEvent();
        }

        private void BackToCatalog()
        {
            currentEngine?.Dispose();
            currentEngine = null;

            if (storyPanel != null) storyPanel.SetActive(false);
            if (catalogPanel != null) catalogPanel.SetActive(true);
        }

        private void RenderEvent()
        {
            if (currentEngine == null) return;

            // Event ID
            if (eventIdText != null)
                eventIdText.text = currentEngine.CurrentEventId;

            // Event text (paragraphs)
            if (proseText != null)
            {
                proseText.text = currentEngine.CurrentEventText;
            }

            // Choices
            if (choicesContainer != null && choiceButtonPrefab != null)
            {
                foreach (Transform child in choicesContainer)
                    Destroy(child.gameObject);

                // If the story is complete, show "Story Complete" and hide choices
                if (currentEngine.IsStoryComplete)
                {
                    var completeObj = new GameObject("StoryCompleteText");
                    completeObj.AddComponent<RectTransform>();
                    var completeText = completeObj.AddComponent<TMP_Text>();
                    completeText.text = "── Story Complete ──";
                    completeText.alignment = TextAnchor.MiddleCenter;
                    completeObj.transform.SetParent(choicesContainer, false);
                }
                else
                {
                    string[] choices = currentEngine.CurrentChoices;
                    for (int i = 0; i < choices.Length; i++)
                    {
                        int choiceIdx = i;
                        var btnObj = Instantiate(choiceButtonPrefab, choicesContainer);
                        var btnText = btnObj.GetComponentInChildren<TMP_Text>();
                        if (btnText != null)
                            btnText.text = $"{i + 1}. {choices[i]}";

                        var btn = btnObj.GetComponent<Button>();
                        if (btn != null)
                            btn.onClick.AddListener(() => OnChoiceSelected(choiceIdx));
                    }
                }
            }

            // Stats
            if (statsText != null)
            {
                var stats = currentEngine.GetStats();
                var tags = currentEngine.GetStoryTags();
                var runtimeTags = currentEngine.GetTags();
                var flags = currentEngine.GetFlags();

                var sb = new StringBuilder();
                sb.AppendLine("Stats:");
                foreach (var kv in stats)
                    sb.AppendLine($"  {kv.Key}: {kv.Value}");
                sb.AppendLine();
                sb.AppendLine("Story Tags: " + string.Join(", ", tags));
                sb.AppendLine("Runtime Tags: " + string.Join(", ", runtimeTags));
                sb.AppendLine("Flags: " + string.Join(", ", flags));
                statsText.text = sb.ToString();
            }
        }

        private void OnChoiceSelected(int index)
        {
            if (currentEngine == null) return;

            // Apply the choice
            currentEngine.MakeChoice(index);

            // Show effect text briefly
            string effectText = currentEngine.LastEffectText;
            if (!string.IsNullOrEmpty(effectText))
            {
                Debug.Log($"[CYOA] Effect: {effectText}");
            }

            // Check if story is now complete
            if (currentEngine.IsStoryComplete)
            {
                Debug.Log("[CYOA] Story complete!");
            }

            // Render the next event
            RenderEvent();
        }

        // ── Save / load ─────────────────────────────────────────────────────

        private void SaveGame()
        {
            if (currentEngine == null) return;

            string json = currentEngine.GetStateJson();
            lastSavedJson = json;

            if (saveTextArea != null)
                saveTextArea.text = json;

            // Also save to file
            string savePath = Path.Combine(Application.persistentDataPath, "cyoa_save.json");
            File.WriteAllText(savePath, json);
            Debug.Log($"Saved game to {savePath}");
        }

        private void LoadGame()
        {
            if (currentEngine == null) return;

            // Try loading from the text area first
            string json = (saveTextArea != null && !string.IsNullOrEmpty(saveTextArea.text))
                ? saveTextArea.text
                : lastSavedJson
                ?? "";

            if (!string.IsNullOrEmpty(json))
            {
                currentEngine.SetStateJson(json);
                RenderEvent();
                Debug.Log("Game state loaded");
            }
            else
            {
                // Try loading from file
                string savePath = Path.Combine(Application.persistentDataPath, "cyoa_save.json");
                if (File.Exists(savePath))
                {
                    string fileJson = File.ReadAllText(savePath);
                    currentEngine.SetStateJson(fileJson);
                    RenderEvent();
                    Debug.Log($"Game state loaded from {savePath}");
                }
                else
                {
                    Debug.LogWarning("No save data found");
                }
            }
        }

        // ── Cleanup ─────────────────────────────────────────────────────────

        private void OnDestroy()
        {
            currentEngine?.Dispose();
            catalog?.Dispose();
        }
    }
}
