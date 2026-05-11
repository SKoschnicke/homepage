+++
title = "Inline Preview for Source File Links in Emacs"
description = "How to show a snipped of a linked source file below a link in org-mode."
date = 2026-05-11
tags = ["emacs", "productivity", "org-mode"]
draft = true
+++

Normal link to source file in org mode:
![](/ox-hugo/emacs-link.png)

Link with preview enabled:
![](/ox-hugo/emacs-link-shown.png)

Org 9.7 ships `org-link-preview` natively, but only for images. We hook into
the same mechanism via the `:preview` link parameter to show a source snippet
for file links that carry a `::N` or `::text` search option. This replaces the
old post-command-hook overlay code: org now manages the overlay lifecycle,
toggling, and clearing — we just produce the content.

Trigger with `SPC m l p` on the link (bound to `my/org-link-preview-here`
below, which calls `org-link-preview` with the prefix arg that makes it work
for links with descriptions too). Use `C-u M-x org-link-preview` to clear.


## Helpers {#helpers}

These pure functions resolve a search option to a line number, derive the
source-block language from the file extension, apply font-lock to a snippet,
and format the result with a 👉 marker on the target line.

```emacs-lisp
(defcustom my/org-link-preview-context-lines 5
  "Number of context lines to show before and after target line."
  :type 'integer
  :group 'org-link)

(defun my/org-link-find-line-by-text (file search-text)
  "Find line number in FILE that contains SEARCH-TEXT.
Returns line number or nil if not found."
  (when (and file (file-exists-p file) search-text)
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (when (search-forward search-text nil t)
        (line-number-at-pos)))))

(defun my/org-link-get-language-from-extension (file)
  "Get org-babel language identifier from FILE extension."
  (let* ((mode (assoc-default file auto-mode-alist 'string-match))
         (mode-name (and mode (symbol-name mode))))
    (if mode-name
        (let ((lang (replace-regexp-in-string "-mode$" "" mode-name)))
          (cond
           ((assoc lang org-src-lang-modes) lang)
           ((string= lang "js") "javascript")
           (t lang)))
      (or (file-name-extension file) "text"))))

(defun my/org-link-apply-syntax-highlighting (content file)
  "Apply syntax highlighting to CONTENT based on FILE's major mode."
  (with-temp-buffer
    (insert content)
    (let ((mode (assoc-default file auto-mode-alist 'string-match)))
      (when (and mode (fboundp mode))
        (ignore-errors
          (funcall mode)
          (font-lock-ensure))))
    (buffer-string)))

(defun my/org-link-format-preview-content (content start-line target-line)
  "Format CONTENT as preview with optional TARGET-LINE highlighting."
  (with-temp-buffer
    (insert content)
    (goto-char (point-min))
    (let ((current-line start-line)
          (result ""))
      (while (not (eobp))
        (let* ((line-text (buffer-substring (point) (line-end-position)))
               (is-target (and target-line (= current-line target-line))))
          (setq result
                (concat result
                        (if is-target
                            (propertize (concat "👉 " line-text "\n") 'face 'hl-line)
                          (concat "   " line-text "\n"))))
          (setq current-line (1+ current-line))
          (forward-line 1)))
      result)))

(defun my/org-link-get-file-preview (file target-line)
  "Get preview text for FILE centered around TARGET-LINE.
Returns propertized string formatted as an org source block, or nil."
  (when (and file (file-exists-p file) target-line)
    (with-temp-buffer
      (insert-file-contents file)
      (let* ((language (my/org-link-get-language-from-extension file))
             (start-line (max 1 (- target-line my/org-link-preview-context-lines)))
             (end-line (+ target-line my/org-link-preview-context-lines)))
        (goto-char (point-min))
        (forward-line (1- start-line))
        (let* ((content-start (point))
               (content-end (progn (forward-line (- end-line start-line -1)) (point)))
               (content (buffer-substring content-start content-end))
               (highlighted (my/org-link-apply-syntax-highlighting content file))
               (formatted (my/org-link-format-preview-content
                           highlighted start-line target-line))
               (line-info (format " :file %s :line %d"
                                  (file-name-nondirectory file) target-line)))
          (concat (propertize (format "#+begin_src %s%s\n" language line-info)
                              'face 'org-block-begin-line)
                  formatted
                  (propertize "#+end_src" 'face 'org-block-end-line)))))))
```


## Hook into `org-link-preview` {#hook-into-org-link-preview}

A `:preview` function receives `(OV PATH LINK)` — it should configure the
overlay OV (we use `after-string` to render the snippet below the link
without altering the link itself) and return non-nil to keep the overlay,
nil to have org delete it.

Our wrapper handles file links that carry a search option by producing a
source snippet. For plain file links without a search option we fall back to
the built-in image previewer, so PNGs etc. still inline as before.

```emacs-lisp
(defun my/org-link-preview-source-file (ov path link)
  "Preview a source-file link as a snippet over overlay OV.
Handles file links with a numeric (::42) or text (::needle) search
option. Returns non-nil on success, nil to let org fall back."
  (let ((search-option (org-element-property :search-option link)))
    (when search-option
      (let* ((file (expand-file-name path))
             (line (if (string-match "\\`\\([0-9]+\\)\\'" search-option)
                       (string-to-number (match-string 1 search-option))
                     (my/org-link-find-line-by-text file search-option)))
             (preview (my/org-link-get-file-preview file line)))
        (when preview
          (overlay-put ov 'after-string (concat "\n" preview "\n"))
          (overlay-put ov 'face 'default)
          t)))))

(defun my/org-link-preview-file-dispatch (ov path link)
  "Try source-file preview first, fall back to org's image previewer."
  (or (my/org-link-preview-source-file ov path link)
      (org-link-preview-file ov path link)))

(with-eval-after-load 'ol
  (org-link-set-parameters "file" :preview #'my/org-link-preview-file-dispatch)
  (org-link-set-parameters "attachment" :preview #'my/org-link-preview-file-dispatch))
```


## Convenience command and binding {#convenience-command-and-binding}

By default `org-link-preview` skips links that have a description, which is
most of the source-file links we care about. This wrapper passes the
`include-linked` prefix so the link at point always gets previewed.

```emacs-lisp
(defun my/org-link-preview-here ()
  "Preview the link at point, including links with descriptions."
  (interactive)
  (org-link-preview 1))

(with-eval-after-load 'org
  (spacemacs/set-leader-keys-for-major-mode 'org-mode "lp" 'my/org-link-preview-here))
```
