;;; rustowl.el --- Visualize Ownership and Lifetimes in Rust -*- lexical-binding: t; -*-

;; Copyright (C) cordx56

;; Author: cordx56
;; Keywords: tools lifetime ownership visualization rust

;; Version: 0.4.0
;; Package-Requires: ((emacs "29.1"))
;; URL: https://github.com/cordx56/rustowl

;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:
;; Visualize Ownership and Lifetimes in Rust.

;;; Code:

(require 'cl-lib)
(require 'jsonrpc)
(require 'url-util)

(defvar lsp-mode nil)
(defvar lsp-bridge-mode nil)
(declare-function lsp-workspaces "lsp-mode")
(declare-function lsp-request-async "lsp-mode")
(declare-function lsp--buffer-uri "lsp-mode")
(declare-function lsp-register-client "lsp-mode")
(declare-function make-lsp-client "lsp-mode")
(declare-function lsp-stdio-connection "lsp-mode")
(declare-function lsp-bridge--get-project-path-func "lsp-bridge")

(defgroup rustowl ()
  "Visualize Ownership and Lifetimes in Rust."
  :group 'tools
  :prefix "rustowl-"
  :link '(url-link "https://github.com/cordx56/rustowl"))

(defcustom rustowl-command '("rustowl")
  "Command used to start RustOwl for lsp-bridge buffers."
  :type '(repeat string)
  :group 'rustowl)

(defvar rustowl--bridge-sessions (make-hash-table :test #'equal)
  "RustOwl sessions indexed by project directory.")

(cl-defstruct (rustowl--session (:constructor rustowl--make-session))
  connection ready pending buffers)

(defvar-local rustowl--bridge-session nil
  "RustOwl session associated with this buffer.")
(defvar-local rustowl--bridge-uri nil
  "Document URI opened in the RustOwl session.")
(defvar-local rustowl--generation 0
  "Generation used to discard obsolete cursor responses.")

(defun rustowl--get (object key)
  "Read KEY from a plist or hash-table OBJECT."
  (let ((value (if (hash-table-p object)
                   (gethash (substring (symbol-name key) 1) object)
                 (plist-get object key))))
    (unless (memq value '(:json-false :false)) value)))

(defun rustowl--backend ()
  "Return the active supported backend for this buffer."
  (cond
   ((and (bound-and-true-p lsp-bridge-mode)
         (not (bound-and-true-p lsp-bridge-remote-file-flag))
         buffer-file-name (not (file-remote-p buffer-file-name)))
    'lsp-bridge)
   ((and (bound-and-true-p lsp-mode) (lsp-workspaces)) 'lsp-mode)))

(defun rustowl--file-uri (file)
  "Convert local FILE to a percent-encoded URI."
  (concat "file://"
          (mapconcat #'url-hexify-string
                     (split-string (expand-file-name file) "/") "/")))

(defun rustowl--bridge-root ()
  "Find the project directory using lsp-bridge or Cargo."
  (file-name-as-directory
   (expand-file-name
    (or (and (fboundp 'lsp-bridge--get-project-path-func)
             (lsp-bridge--get-project-path-func buffer-file-name))
        (locate-dominating-file buffer-file-name "Cargo.toml")
        (file-name-directory buffer-file-name)))))

(defun rustowl--bridge-stop (session)
  "Stop SESSION and remove it from the session cache."
  (maphash (lambda (root value)
             (when (eq value session) (remhash root rustowl--bridge-sessions)))
           rustowl--bridge-sessions)
  (setf (rustowl--session-pending session) nil)
  (let ((connection (rustowl--session-connection session)))
    (when (and connection (jsonrpc-running-p connection))
      (when (rustowl--session-ready session)
        (ignore-errors
          (jsonrpc-request connection :shutdown nil :timeout 2)
          (jsonrpc-notify connection :exit nil)))
      (jsonrpc-shutdown connection))))

(defun rustowl--bridge-detach ()
  "Close this buffer's RustOwl document and release its session."
  (when rustowl--bridge-session
    (cl-incf rustowl--generation)
    (let* ((session rustowl--bridge-session)
           (connection (rustowl--session-connection session)))
      (when (and rustowl--bridge-uri (jsonrpc-running-p connection))
        (jsonrpc-notify connection :textDocument/didClose
                        `(:textDocument (:uri ,rustowl--bridge-uri))))
      (setf (rustowl--session-buffers session)
            (delq (current-buffer) (rustowl--session-buffers session)))
      (unless (rustowl--session-buffers session)
        (rustowl--bridge-stop session)))
    (setq rustowl--bridge-session nil rustowl--bridge-uri nil)))

(defun rustowl--bridge-session ()
  "Return or asynchronously start this buffer's RustOwl session."
  (let* ((root (rustowl--bridge-root))
         (session (gethash root rustowl--bridge-sessions)))
    (unless (and session
                 (jsonrpc-running-p (rustowl--session-connection session)))
      (setq session (rustowl--make-session))
      (let* ((default-directory root)
             (connection
              (make-instance
               'jsonrpc-process-connection
               :name (concat "rustowl:" root)
               :process (make-process :name "rustowl" :command rustowl-command
                                      :coding 'utf-8-emacs-unix
                                      :connection-type 'pipe :noquery t
                                      :stderr (get-buffer-create "*rustowl-stderr*"))
               :notification-dispatcher #'ignore)))
        (setf (rustowl--session-connection session) connection)
        (puthash root session rustowl--bridge-sessions)
        (jsonrpc-async-request
         connection :initialize
         `(:processId ,(emacs-pid) :rootUri ,(rustowl--file-uri root)
                      :capabilities ,(make-hash-table))
         :timeout 120
         :success-fn
         (lambda (_result)
           (jsonrpc-notify connection :initialized (make-hash-table))
           (setf (rustowl--session-ready session) t)
           (let ((pending (nreverse (rustowl--session-pending session))))
             (setf (rustowl--session-pending session) nil)
             (mapc #'funcall pending)))
         :error-fn (lambda (err)
                     (rustowl--bridge-stop session)
                     (message "RustOwl initialization failed: %s" err))
         :timeout-fn (lambda ()
                       (rustowl--bridge-stop session)
                       (message "RustOwl initialization timed out")))))
    (unless (eq rustowl--bridge-session session)
      (rustowl--bridge-detach)
      (setq rustowl--bridge-session session)
      (cl-pushnew (current-buffer) (rustowl--session-buffers session))
      (add-hook 'kill-buffer-hook #'rustowl--bridge-detach nil t)
      (add-hook 'change-major-mode-hook #'rustowl--bridge-detach nil t)
      (add-hook 'after-change-functions #'rustowl--bridge-changed nil t)
      (add-hook 'lsp-bridge-mode-hook #'rustowl--bridge-mode-changed nil t))
    session))

(defun rustowl--bridge-mode-changed ()
  "Release RustOwl when lsp-bridge is disabled."
  (unless (bound-and-true-p lsp-bridge-mode)
    (rustowl--bridge-detach)
    (rustowl-clear-overlays)))

(defun rustowl--bridge-changed (&rest _args)
  "Invalidate analysis after a buffer edit."
  (cl-incf rustowl--generation)
  (rustowl-clear-overlays)
  (when (and rustowl--bridge-uri rustowl--bridge-session
             (jsonrpc-running-p
              (rustowl--session-connection rustowl--bridge-session)))
    ;; RustOwl analyzes saved files.  didChange only invalidates its cache.
    (jsonrpc-notify
     (rustowl--session-connection rustowl--bridge-session)
     :textDocument/didChange
     `(:textDocument (:uri ,rustowl--bridge-uri :version ,rustowl--generation)
                     :contentChanges []))))

(defun rustowl--bridge-request (method params callback)
  "Send METHOD with PARAMS and CALLBACK through the RustOwl session."
  (let* ((session (rustowl--bridge-session))
         (buffer (current-buffer))
         (send
          (lambda ()
            (when (buffer-live-p buffer)
              (with-current-buffer buffer
                (when (and (eq rustowl--bridge-session session)
                           (eq (rustowl--backend) 'lsp-bridge))
                  (let ((connection (rustowl--session-connection session)))
                    (unless rustowl--bridge-uri
                      (setq rustowl--bridge-uri (rustowl--file-uri buffer-file-name))
                      (jsonrpc-notify
                       connection :textDocument/didOpen
                       `(:textDocument
                         (:uri ,rustowl--bridge-uri :languageId "rust" :version 0
                               :text ,(save-restriction
                                        (widen) (buffer-string))))))
                    (jsonrpc-async-request connection (intern method) params
                                           :success-fn callback))))))))
    (if (rustowl--session-ready session)
        (funcall send)
      (push send (rustowl--session-pending session)))))

(defun rustowl--request (method params callback)
  "Send METHOD with PARAMS and CALLBACK using the active backend."
  (let* ((buffer (current-buffer))
         (generation rustowl--generation)
         (tick (buffer-chars-modified-tick))
         (position (point))
         (backend (rustowl--backend))
         (receive
          (lambda (response)
            (when (buffer-live-p buffer)
              (with-current-buffer buffer
                (when (and (eq backend (rustowl--backend))
                           (= generation rustowl--generation)
                           (= tick (buffer-chars-modified-tick))
                           (= position (point)))
                  (funcall callback response)))))))
    (pcase backend
      ('lsp-mode (lsp-request-async method params receive :mode 'current))
      ('lsp-bridge (rustowl--bridge-request method params receive)))))

;; Register the LSP client.
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
  "Send a `rustowl/analyze' request for the current buffer."
  (rustowl--request "rustowl/analyze" (make-hash-table) #'ignore))

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
  (when (rustowl--backend)
    (rustowl--request
     "rustowl/cursor" params
     (lambda (response)
       (let ((decorations (rustowl--get response :decorations)))
         (mapc
          (lambda (deco)
            (let* ((type (rustowl--get deco :type))
                   (range (rustowl--get deco :range))
                   (start (rustowl--get range :start))
                   (end (rustowl--get range :end))
                   (start-pos
                    (rustowl-line-col-to-pos (rustowl--get start :line) (rustowl--get start :character)))
                   (end-pos (rustowl-line-col-to-pos (rustowl--get end :line) (rustowl--get end :character)))
                   (overlapped (rustowl--get deco :overlapped)))
              (if (not overlapped)
                  (cond
                   ((equal type "definitely_live")
                    (rustowl-underline start-pos end-pos "#00cc00" nil))
                   ((equal type "maybe_initialized")
                    (rustowl-underline start-pos end-pos "#00cc00" t))
                   ((equal type "imm_borrow")
                    (rustowl-underline start-pos end-pos "#0000cc" nil))
                   ((equal type "mut_borrow")
                    (rustowl-underline start-pos end-pos "#cc00cc" nil))
                   ((or (equal type "move") (equal type "call"))
                    (rustowl-underline start-pos end-pos "#cccc00" nil))
                   ((or (equal type "shared_mut") (equal type "outlive"))
                    (rustowl-underline start-pos end-pos "#cc0000" t))))))
          decorations))))))

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
  (when (and (rustowl--backend)
             (not (and (eq (rustowl--backend) 'lsp-bridge)
                       (buffer-modified-p))))
    (let ((line (rustowl-line-number-at-pos))
          (column (rustowl-current-column))
          (uri (if (eq (rustowl--backend) 'lsp-bridge)
                   (rustowl--file-uri buffer-file-name)
                 (lsp--buffer-uri))))
      (rustowl-cursor `(:position (:line ,line :character ,column) :document (:uri ,uri))))))

;;;###autoload
(defvar-local rustowl-cursor-timer nil
  "Timer object for rustowl cursor overlays.")

;;;###autoload
(defvar rustowl-cursor-timeout 2.0
  "Idle seconds before showing cursor overlays.")

;;;###autoload
(defun rustowl-reset-cursor-timer ()
  "Reset RustOwl's idle timer for overlays."
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer))
  (cl-incf rustowl--generation)
  (rustowl-clear-overlays)
  (setq rustowl-cursor-timer
        (run-with-idle-timer rustowl-cursor-timeout nil #'rustowl--cursor-call-in
                             (current-buffer))))

(defun rustowl--cursor-call-in (buffer)
  "Call `rustowl-cursor-call' in BUFFER when it is still current."
  (when (and (buffer-live-p buffer) (eq buffer (current-buffer)))
    (with-current-buffer buffer
      (rustowl-cursor-call))))

;;;###autoload
(defun rustowl-enable-cursor ()
  "Enable RustOwl cursor overlays."
  (add-hook 'kill-buffer-hook #'rustowl-disable-cursor nil t)
  (add-hook 'change-major-mode-hook #'rustowl-disable-cursor nil t)
  (add-hook 'post-command-hook #'rustowl-reset-cursor-timer nil t))

;;;###autoload
(defun rustowl-disable-cursor ()
  "Disable RustOwl cursor overlays."
  (cl-incf rustowl--generation)
  (remove-hook 'post-command-hook #'rustowl-reset-cursor-timer t)
  (when rustowl-cursor-timer
    (cancel-timer rustowl-cursor-timer)
    (setq rustowl-cursor-timer nil))
  (rustowl-clear-overlays))

(define-obsolete-function-alias 'enable-rustowl-cursor #'rustowl-enable-cursor "0.4.1")
(define-obsolete-function-alias 'disable-rustowl-cursor #'rustowl-disable-cursor "0.4.1")

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

(defvar-local rustowl-overlays nil
  "List of currently active RustOwl overlays.")

(defun rustowl-underline (start end color wavy)
  "Underline region between START and END with COLOR.
If WAVY is non-nil, use a wavy underline, otherwise a straight line."
  (let ((overlay (make-overlay start end)))
    (if wavy
        (overlay-put overlay 'face `(:underline (:color ,color :style wave)))
      (overlay-put overlay 'face `(:underline (:color ,color :style line))))
    (push overlay rustowl-overlays)
    overlay))

(defun rustowl-clear-overlays ()
  "Remove all RustOwl overlays."
  (interactive)
  (mapc #'delete-overlay rustowl-overlays)
  (setq rustowl-overlays nil))

(provide 'rustowl)
;;; rustowl.el ends here
