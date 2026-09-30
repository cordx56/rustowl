;;; rustowl.el --- Visualize Ownership and Lifetimes in Rust -*- lexical-binding: t; -*-

;; Copyright (C) cordx56

;; Author: cordx56
;; Keywords: tools lifetime ownership visualization rust

;; Version: 0.4.0
;; Package-Requires: ((emacs "28.1") (lsp-mode "9.0.0"))
;; URL: https://github.com/cordx56/rustowl

;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:
;; Visualize Ownership and Lifetimes in Rust.

;;; Code:

(require 'lsp-mode)

(defgroup rustowl ()
  "Visualize Ownership and Lifetimes in Rust."
  :group 'tools
  :prefix "rustowl-"
  :link '(url-link "https://github.com/cordx56/rustowl"))

;;;###autoload
(lsp-register-client
 (make-lsp-client
  :new-connection
  (lsp-stdio-connection '("rustowl"))
  :major-modes
  '(rust-mode rust-ts-mode rustic-mode)
  :server-id 'rustowl
  :priority -1
  :add-on? t))

;; Analyze on save
(defun rustowl--analyze-request ()
  "Send a rustowl/analyze request to the LSP server for the current buffer."
  (when (and (bound-and-true-p lsp-mode) (lsp-workspaces))
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
  "Request and visualize Rust ownership/lifetime overlays for PARAMS."
  (when (and (bound-and-true-p lsp-mode) (lsp-workspaces))
    (lsp-request-async
     "rustowl/cursor" params
     (lambda (response)
       (let ((decorations (lsp-get response :decorations)))
         (mapc
          (lambda (deco)
            (let* ((type (lsp-get deco :type))
                   (range (lsp-get deco :range))
                   (start (lsp-get range :start))
                   (end (lsp-get range :end))
                   (start-pos
                    (rustowl-line-col-to-pos
                     (lsp-get start :line)
                     (lsp-get start :character)))
                   (end-pos
                    (rustowl-line-col-to-pos
                     (lsp-get end :line) (lsp-get end :character)))
                   (overlapped (lsp-get deco :overlapped)))
              (if (not overlapped)
                  (cond
                   ((equal type "definitely_live")
                    (rustowl-underline
                     start-pos end-pos "#00cc00" nil))
                   ((equal type "maybe_initialized")
                    (rustowl-underline start-pos end-pos "#00cc00" t))
                   ((equal type "imm_borrow")
                    (rustowl-underline
                     start-pos end-pos "#0000cc" nil))
                   ((equal type "mut_borrow")
                    (rustowl-underline
                     start-pos end-pos "#cc00cc" nil))
                   ((or (equal type "move") (equal type "call"))
                    (rustowl-underline
                     start-pos end-pos "#cccc00" nil))
                   ((or (equal type "shared_mut")
                        (equal type "outlive"))
                    (rustowl-underline
                     start-pos end-pos "#cc0000" t))))))
          decorations)))
     :mode 'current)))

(defun rustowl-line-number-at-pos ()
  "Return the 0-based line number at point."
  (save-restriction
    (widen)
    (save-excursion
      (let ((inhibit-field-text-motion t))
        (1- (line-number-at-pos))))))

(defun rustowl-current-column ()
  "Return the current column at point."
  (save-restriction
    (widen)
    (let ((inhibit-field-text-motion t))
      (- (point) (line-beginning-position)))))

(defun rustowl-cursor-call ()
  "Call RustOwl for current cursor position."
  (when (and (bound-and-true-p lsp-mode) (lsp-workspaces))
    (let ((line (rustowl-line-number-at-pos))
          (column (rustowl-current-column))
          (uri (lsp--buffer-uri)))
      (rustowl-cursor
       `(:position
         (:line ,line :character ,column)
         :document (:uri ,uri))))))

;;;###autoload
(defvar rustowl-cursor-timer nil
  "Timer object for rustowl cursor overlays.")

;;;###autoload
(defvar rustowl-cursor-timeout 2.0
  "Idle seconds before showing cursor overlays.")

;;;###autoload
(defun rustowl-reset-cursor-timer ()
  "Reset RustOwl's idle timer for overlays."
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer))
  (rustowl-clear-overlays)
  (setq rustowl-cursor-timer
        (run-with-idle-timer
         rustowl-cursor-timeout nil #'rustowl--cursor-call-in
         (current-buffer))))

(defun rustowl--cursor-call-in (buffer)
  "Call `rustowl-cursor-call' in BUFFER if it is still live and current."
  (when (and (buffer-live-p buffer) (eq buffer (current-buffer)))
    (with-current-buffer buffer
      (rustowl-cursor-call))))

;;;###autoload
(defun rustowl-enable-cursor ()
  "Enable RustOwl overlay updates on cursor move."
  (add-hook 'post-command-hook #'rustowl-reset-cursor-timer nil t))

;;;###autoload
(defun rustowl-disable-cursor ()
  "Disable RustOwl overlay updates."
  (remove-hook 'post-command-hook #'rustowl-reset-cursor-timer t)
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer)
    (setq rustowl-cursor-timer nil))
  (rustowl-clear-overlays))

(define-obsolete-function-alias
  'enable-rustowl-cursor #'rustowl-enable-cursor "0.4.1")
(define-obsolete-function-alias
  'disable-rustowl-cursor #'rustowl-disable-cursor "0.4.1")

;; Automatically enable cursor-based highlighting for Rust buffers
(add-hook 'rust-mode-hook #'rustowl-enable-cursor)
(add-hook 'rust-ts-mode-hook #'rustowl-enable-cursor)
(add-hook 'rustic-mode-hook #'rustowl-enable-cursor)

;; RustOwl visualization
(defun rustowl-line-col-to-pos (line col)
  "Convert LINE and COL to buffer position.
LINE and COL are 0-based (LSP compatible);
if either is negative (< 0), signal an error.
If LINE is past the last line, return (point-max).
If COL is past end of line, clamp to end of line."
  (when (or (< line 0) (< col 0))
    (error "Negative line or column: %s %s" line col))
  (save-restriction
    (widen)
    (save-excursion
      (let ((inhibit-field-text-motion t))
        (goto-char (point-min))
        (let ((max-line (line-number-at-pos (point-max))))
          (if (>= line max-line)
              (point-max)
            (forward-line line)
            (let ((bol (point))
                  (eol (line-end-position)))
              (goto-char bol)
              (forward-char (min col (- eol bol)))
              (point))))))))

(defvar rustowl-overlays nil
  "List of currently active RustOwl overlays.")

(defun rustowl-underline (start end color wavy)
  "Underline region between START and END with COLOR.
If WAVY is non-nil, use a wavy underline, otherwise a straight line."
  (let ((overlay (make-overlay start end)))
    (if wavy
        (overlay-put
         overlay 'face `(:underline (:color ,color :style wave)))
      (overlay-put
       overlay 'face `(:underline (:color ,color :style line))))
    (push overlay rustowl-overlays)
    overlay))

(defun rustowl-clear-overlays ()
  "Remove all RustOwl overlays."
  (interactive)
  (mapc #'delete-overlay rustowl-overlays)
  (setq rustowl-overlays nil))

(provide 'rustowl)
;;; rustowl.el ends here
