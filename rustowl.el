;;; rustowl.el --- Visualize Ownership and Lifetimes in Rust -*- lexical-binding: t; -*-

;; Copyright (C) cordx56

;; Author: cordx56
;; Keywords: tools lifetime ownership visualization rust

;; Version: 0.3.4
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
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("rustowl"))
    :major-modes '(rust-mode)
    :server-id 'rustowl
    :priority -1
    :add-on? t)))

(defun rustowl-cursor (params)
  "Request and visualize Rust ownership/lifetime overlays for PARAMS."
  (lsp-request-async
   "rustowl/cursor" params
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
                   (gethash "line" end) (gethash "character" end)))
                 (overlapped (gethash "overlapped" deco)))
            (unless overlapped
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
   :mode 'current))

(defun rustowl-line-number-at-pos ()
  "Return the line number at point."
  (save-excursion
    (goto-char (point))
    (count-lines (point-min) (line-beginning-position))))

(defun rustowl-current-column ()
  "Return the current column at point."
  (save-excursion
    (let ((start (point)))
      (move-beginning-of-line 1)
      (- start (point)))))

(defun rustowl-cursor-call ()
  "Call RustOwl for current cursor position."
  (let ((line (rustowl-line-number-at-pos))
        (column (rustowl-current-column))
        (uri (lsp--buffer-uri)))
    (rustowl-cursor
     `(:position
       (:line ,line :character ,column)
       :document (:uri ,uri)))))

;;;###autoload
(defvar rustowl-cursor-timer nil
  "Timer object for rustowl cursor overlays.")

;;;###autoload
(defvar rustowl-cursor-timeout 2
  "Idle seconds before showing cursor overlays.")

;;;###autoload
(defun rustowl-reset-cursor-timer ()
  "Reset RustOwl's idle timer for overlays."
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer))
  (rustowl-clear-overlays)
  (setq rustowl-cursor-timer
        (run-with-idle-timer
         rustowl-cursor-timeout nil #'rustowl-cursor-call)))

;;;###autoload
(defun rustowl-enable-cursor ()
  "Enable RustOwl overlay updates on cursor move."
  (add-hook 'post-command-hook #'rustowl-reset-cursor-timer))

;;;###autoload
(defun rustowl-disable-cursor ()
  "Disable RustOwl overlay updates."
  (remove-hook 'post-command-hook #'rustowl-reset-cursor-timer)
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer)
    (setq rustowl-cursor-timer nil)))

(defun rustowl-line-col-to-pos (line col)
  "Convert LINE and COL to buffer position.
If LINE or COL is negative, signal an error.
If LINE is past the last line, return (point-max).
If COL is past end of line, clamp to end of line."
  (when (or (< line 0) (< col 0))
    (error "rustowl-line-col-to-pos: negative line or column"))
  (save-excursion
    (goto-char (point-min))
    (let ((max-line (count-lines (point-min) (point-max))))
      (if (>= line max-line)
          (point-max)
        (forward-line line)
        (let ((bol (point))
              (eol (line-end-position)))
          (goto-char bol)
          (forward-char (min col (- eol bol)))
          (point))))))

(defvar rustowl-overlays nil
  "List of currently active RustOwl overlays.")

(defun rustowl-underline (start end color)
  "Underline region from START to END with COLOR."
  (let ((overlay (make-overlay start end)))
    (overlay-put
     overlay 'face `(:underline (:color ,color :style wave)))
    (push overlay rustowl-overlays)
    overlay))

(defun rustowl-clear-overlays ()
  "Remove all RustOwl overlays."
  (interactive)
  (mapc #'delete-overlay rustowl-overlays)
  (setq rustowl-overlays nil))

(provide 'rustowl)
;;; rustowl.el ends here
