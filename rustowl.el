;;; rustowl.el --- Visualize Ownership and Lifetimes in Rust -*- lexical-binding: t; -*-

;; Copyright (C) cordx56

;; Author: cordx56
;; Keywords: tools

;; Version: 0.2.0
;; Package-Requires: ((emacs "24.1") (lsp-mode "9.0.0"))
;; URL: https://github.com/cordx56/rustowl

;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;;; Code:

(require 'lsp-mode)

(defgroup rustowl ()
  "Visualize Ownership and Lifetimes in Rust"
  :group 'tools
  :prefix "rustowl-"
  :link '(url-link "https://github.com/cordx56/rustowl"))

;;;###autoload
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("rustowl"))
    :major-modes '(rust-mode rust-ts-mode rustic-mode)
     :server-id 'rustowl
     :priority -1
     :add-on? t)))

;; Analyze on save
(defun rustowl--analyze-request ()
  "Send a rustowl/analyze request to the LSP server for the current buffer."
  (when (and (bound-and-true-p lsp-mode)
             (lsp-workspaces))
    (lsp-request-async
     "rustowl/analyze"
     (make-hash-table)
     #'ignore
     :mode 'current)))

(defun rustowl-enable-analyze-on-save ()
  "Enable sending rustowl/analyze on save in this buffer."
  (add-hook 'after-save-hook #'rustowl--analyze-request nil t))

(defun rustowl-disable-analyze-on-save ()
  "Disable sending rustowl/analyze on save in this buffer."
  (remove-hook 'after-save-hook #'rustowl--analyze-request t))

;; Automatically enable for Rust buffers
(add-hook 'rust-mode-hook #'rustowl-enable-analyze-on-save)
(add-hook 'rust-ts-mode-hook #'rustowl-enable-analyze-on-save)
(add-hook 'rustic-mode-hook #'rustowl-enable-analyze-on-save)

(defun rustowl-cursor (params)
  "Send rustowl/cursor request if LSP is active in this buffer."
  (when (and (bound-and-true-p lsp-mode)
             (lsp-workspaces))
    (lsp-request-async
     "rustowl/cursor"
     params
     (lambda (response)
       (let ((decorations (gethash "decorations" response)))
         (mapc
          (lambda (deco)
            (let* ((type (gethash "type" deco))
                   (start (gethash "start" (gethash "range" deco)))
                   (end (gethash "end" (gethash "range" deco)))
                   (start-pos
                    (rustowl-line-col-to-pos
                     (gethash "line" start)
                     (gethash "character" start)))
                   (end-pos
                    (rustowl-line-col-to-pos
                     (gethash "line" end)
                     (gethash "character" end)))
                   (overlapped (gethash "overlapped" deco)))
              (if (not overlapped)
                  (cond
                   ((equal type "lifetime")
                    (rustowl-underline start-pos end-pos "#00cc00"))
                   ((equal type "imm_borrow")
                    (rustowl-underline start-pos end-pos "#0000cc"))
                   ((equal type "mut_borrow")
                    (rustowl-underline start-pos end-pos "#cc00cc"))
                   ((or (equal type "move") (equal type "call"))
                    (rustowl-underline start-pos end-pos "#cccc00"))
                   ((equal type "outlive")
                    (rustowl-underline start-pos end-pos "#cc0000"))))))
          decorations)))
     :mode 'current)))


(defun rustowl-line-number-at-pos ()
  (save-excursion
    (goto-char (point))
    (count-lines (point-min) (line-beginning-position))))
(defun rustowl-current-column ()
  (save-excursion
    (let ((start (point)))
      (move-beginning-of-line 1)
      (- start (point)))))

(defun rustowl-cursor-call ()
  (when (and (bound-and-true-p lsp-mode)
             (lsp-workspaces))
    (let* ((line (rustowl-line-number-at-pos))
           (column (rustowl-current-column))
           (uri (lsp--buffer-uri))
           (pos (let ((ht (make-hash-table :test 'equal)))
                  (puthash "line" line ht)
                  (puthash "character" column ht)
                  ht))
           (doc (let ((ht (make-hash-table :test 'equal)))
                  (puthash "uri" uri ht)
                  ht))
           (params (let ((ht (make-hash-table :test 'equal)))
                     (puthash "position" pos ht)
                     (puthash "document" doc ht)
                     ht)))
      (rustowl-cursor params))))

;;;###autoload
(defvar rustowl-cursor-timer nil)
;;;###autoload
(defvar rustowl-cursor-timeout 2.0)

;;;###autoload
(defun rustowl-reset-cursor-timer ()
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer))
  (rustowl-clear-overlays)
  (setq rustowl-cursor-timer
        (run-with-idle-timer rustowl-cursor-timeout nil #'rustowl-cursor-call)))

;;;###autoload
(defun enable-rustowl-cursor ()
  (add-hook 'post-command-hook #'rustowl-reset-cursor-timer nil t))

;;;###autoload
(defun disable-rustowl-cursor ()
  (remove-hook 'post-command-hook #'rustowl-reset-cursor-timer t)
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer)
    (setq rustowl-cursor-timer nil)))

;; Automatically enable cursor-based highlighting for Rust buffers
(add-hook 'rust-mode-hook #'enable-rustowl-cursor)
(add-hook 'rust-ts-mode-hook #'enable-rustowl-cursor)
(add-hook 'rustic-mode-hook #'enable-rustowl-cursor)

;; RustOwl visualization
(defun rustowl-line-col-to-pos (line col)
  (save-excursion
    (goto-char (point-min))
    (forward-line line)
    (move-to-column col)
    (point)))

(defvar rustowl-overlays nil)

(defun rustowl-underline (start end color)
  (let ((overlay (make-overlay start end)))
    (overlay-put overlay 'face `(:underline (:color ,color :style wave)))
    (push overlay rustowl-overlays)
    overlay))

(defun rustowl-clear-overlays ()
  (interactive)
  (mapc #'delete-overlay rustowl-overlays)
  (setq rustowl-overlays nil))

(provide 'rustowl)
;;; rustowl.el ends here
