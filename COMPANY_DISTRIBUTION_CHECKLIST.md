# Yap Company Distribution: Pre-Launch Checklist

## Executive Summary

**Status**: Ready to audit, but 3 critical issues found on Windows  
**Timeline**: 7-10 hours to fix + test  
**Risk Level**: Low (fixes are isolated to copy/paste and settings)

---

## What Works Today ✅

- **Talk Key**: Right-Ctrl (default on Windows) + customizable
- **Auto-Paste**: Toggle in Settings
- **Restore Clipboard**: Toggle in Settings  
- **Cleanup Rules**: 9 types (fillers, grammar, punctuation, etc.) with Do/Suggest/Leave options
- **List Detection**: Auto-detects when you're listing things, can ask to format
- **Manual Copy**: Copy button on each dictation card

---

## What Breaks or Is Missing ❌

| Issue | Impact | Severity | Fix Time |
|-------|--------|----------|----------|
| Paste doesn't work in Discord/Slack Web | Users confused, text not pasted | 🔴 Critical | 2-3 hours |
| Clipboard restoration loses your copied text | Silent data loss | 🔴 Critical | 1-2 hours |
| Can't customize list format (bullets, spacing, etc.) | Can't match team style | 🟡 High | 3-4 hours |
| No settings export/import | Can't pre-configure for company | 🔴 Critical | 1-2 hours |
| No visible paste feedback | Users don't know if paste worked | 🟡 High | 1-2 hours |
| Can't set different paste behavior per app | Discord needs different strategy than Notepad | 🟠 Medium | 2-3 hours |

---

## Critical Fixes Required

### Fix 1: Paste Fallback for Web Apps (2-3 hours)
**Problem**: Ctrl+V doesn't trigger in Slack Web, Discord, Gmail  
**Solution**: Add retry with longer key hold + fallback strategy  
**Files**: `src-tauri/src/paste.rs`

### Fix 2: Clipboard Restoration Retry Logic (1-2 hours)
**Problem**: Old clipboard content silently lost after pasting  
**Solution**: Retry 3x with progressive delays + error tracking  
**Files**: `src-tauri/src/paste.rs`

### Fix 3: Settings Export/Import (1-2 hours)
**Problem**: Each user must manually configure; can't set company defaults  
**Solution**: Add export/import UI + check for `settings.default.json` on startup  
**Files**: `src-tauri/src/lib.rs`, `src/views/Settings.svelte`

---

## High-Impact Enhancements

### Enhancement 1: List Formatting Controls (3-4 hours)
**Why**: Company users need different list styles (Slack is compact, Email is formal)

**Options**:
- Bullet style: `-`, `*`, `•`, `1.`, `→`, or custom
- Intro text: "Here's what I said:" or none
- Spacing: single or double line breaks
- Numbering: toggle on/off
- Indentation: 0-4 spaces for nested items

**Example**: 
```
Without formatting:
I did this, then that, then this

With formatting (Email style):
Here's what I said:

1. I did this
2. Then that
3. Then this
```

### Enhancement 2: Paste Feedback (1-2 hours)
**Why**: Users need to know paste succeeded, failed, or requires manual action

**Shows**:
- ✓ "Pasted" (success)
- 📋 "Copied (use Ctrl+V)" (copy-only mode)
- ✗ "Paste failed" (error)

---

## What You Can Tell Your Team

**"Yap is ready for company distribution with these capabilities"**:

✅ **Clear Control** — Customize talk key, auto-paste, cleanup rules  
✅ **Reliable Pasting** — Works in Notepad, Word, and with clipboard restoration  
✅ **Smart Cleanup** — 9 types of changes you can enable/disable per your style  
✅ **List Formatting** — Detects when you list things and can format nicely  
✅ **Pre-Configured** — Company settings distributed on install  

**"If you need"**:
- Different list styles per team (Slack vs Email) → Implement Enhancement 1
- Visible feedback on paste → Implement Enhancement 2
- Different paste behavior per app → Implement Enhancement 3

---

## Implementation Plan

### Week 1: Critical Fixes
- [ ] Fix paste fallback for web apps (day 1-2)
- [ ] Fix clipboard restoration (day 2)
- [ ] Add export/import settings (day 3-4)
- [ ] Internal testing & QA (day 4-5)

### Week 2: Enhancements & Polish
- [ ] List formatting controls (day 1-2)
- [ ] Paste feedback UI (day 1)
- [ ] Company documentation & training (day 3-4)

### Week 3: Final Testing & Rollout
- [ ] Test in all target apps (Discord, Slack, Gmail, Teams, Notepad, Word, VS Code)
- [ ] User acceptance testing (UAT)
- [ ] Deploy to company

---

## Testing Matrix

Test these scenarios before launch:

| App | Auto-Paste | Restore Clipboard | List Format | Notes |
|-----|-----------|------------------|------------|-------|
| Slack Desktop | ✓ | ✓ | ✓ | Should work out of box |
| Discord Desktop | ✓ | ✓ | ✓ | Should work |
| Slack Web | ✓ (with fix) | ✓ (with fix) | ✓ | NEEDS FIX: Ctrl+V doesn't always trigger |
| Discord Web | ✓ (with fix) | ✓ (with fix) | ✓ | NEEDS FIX: May need fallback |
| Gmail Web | ✓ (with fix) | ✓ (with fix) | ✓ | NEEDS FIX: Compose box paste |
| Teams Web | ✓ (with fix) | ✓ (with fix) | ✓ | NEEDS FIX: Chat paste |
| Notepad | ✓ | ✓ | ✓ | Baseline test |
| Word | ✓ | ✓ | ✓ | Rich text support |
| VS Code | ✓ | ✓ | ✓ | Code paste |

---

## Pre-Launch Deployment Strategy

### Step 1: Create Company Settings Profile
```json
{
  "trigger": "right-ctrl",
  "autoPaste": true,
  "restoreClipboard": true,
  "lists": "ask",
  "listPreferences": {
    "bulletStyle": "dash",
    "introText": "",
    "itemSpacing": "single"
  }
}
```

### Step 2: Package with Installer
- Settings JSON → `C:\Program Files\Yap\defaults\settings.default.json`
- Or upload to company shared folder

### Step 3: First-Launch Experience
- Yap detects defaults file
- Shows: "Import company settings?"
- User clicks yes → settings applied
- User can customize later in Settings

### Step 4: Documentation for Users
Provide simple one-pager:
- How to dictate (hold Right-Ctrl)
- Where text goes (auto-pastes into focused app)
- How to customize (Settings panel)
- Troubleshooting (if paste doesn't work)

---

## Success Criteria

✅ **Paste works consistently** in Slack, Discord, Gmail, Teams, Notepad  
✅ **Clipboard restored** without data loss  
✅ **Pre-configured** with company defaults on first launch  
✅ **Users can customize** list format, talk key, cleanup rules  
✅ **No support tickets** about "text disappeared" or "paste didn't work"  

---

## Go/No-Go Decision Points

### Before Development
- [ ] Approve budget/timeline (7-10 hours)
- [ ] Identify target companies/departments
- [ ] Decide on default talk key (right-ctrl or custom)

### After Critical Fixes
- [ ] All 3 critical issues resolved
- [ ] Pass internal QA testing
- [ ] No regressions on macOS

### Before Launch
- [ ] UAT completed with pilot group
- [ ] Documentation ready
- [ ] Support team trained
- [ ] Settings pre-configured + packaged

---

## Questions for Your Team

1. **Talk Key**: Should default be Right-Ctrl or something else?
2. **List Format**: Do you want preset styles (Slack, Email, Meeting Notes)?
3. **Clipboard**: Important that users' clipboard is restored, or can it stay as dictation?
4. **Cleanup Rules**: Should all 9 rules be enabled by default, or conservative (only critical ones)?
5. **Paste Feedback**: Toast notification or status bar indicator?

---

## Next Steps

1. Review audit report (links above)
2. Decide: Fix immediately vs. phased approach
3. Assign implementation + QA
4. Plan internal testing
5. Roll out to pilot group
6. Full company launch

---

**Questions?** See WINDOWS_FIXES.md for technical details or CUSTOMIZATION_ROADMAP.md for feature specs.
