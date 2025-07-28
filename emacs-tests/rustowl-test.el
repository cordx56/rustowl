;;; rustowl-test.el --- ERT tests for rustowl.el -*- lexical-binding: t; -*-

;;; Code:

(require 'ert)
(require 'rustowl)

;; Test rustowl-line-number-at-pos
(ert-deftest rustowl-test-line-number-at-pos ()
  (with-temp-buffer
    (insert "line1\nline2\nline3")
    (goto-char (point-min))
    (should (= (rustowl-line-number-at-pos) 0))
    (forward-line 1)
    (should (= (rustowl-line-number-at-pos) 1))
    (forward-line 1)
    (should (= (rustowl-line-number-at-pos) 2))))

;; Test rustowl-current-column
(ert-deftest rustowl-test-current-column ()
  (with-temp-buffer
    (insert "abc\ndef\nghi")
    (goto-char (point-min))
    (forward-char 2)
    (should (= (rustowl-current-column) 2))
    (forward-line 1)
    (should (= (rustowl-current-column) 0))))

;; Test rustowl-line-col-to-pos
(ert-deftest rustowl-test-line-col-to-pos ()
  (with-temp-buffer
    (insert "abc\ndef\nghi")
    (let ((pos (rustowl-line-col-to-pos 1 2)))
      (goto-char pos)
      (should (= (rustowl-line-number-at-pos) 1))
      (should (= (rustowl-current-column) 2)))))

;; Test rustowl-underline and rustowl-clear-overlays
(ert-deftest rustowl-test-underline-and-clear ()
  (with-temp-buffer
    (insert "abcdef")
    (let ((ov (rustowl-underline 1 4 "#ff0000")))
      (should (overlayp ov))
      (should (memq ov rustowl-overlays))
      (should
       (equal
        (overlay-get ov 'face)
        '(:underline (:color "#ff0000" :style wave))))
      (rustowl-clear-overlays)
      (should (null rustowl-overlays)))))

;; Test rustowl-underline edge cases
(ert-deftest rustowl-test-underline-edge-cases ()
  (with-temp-buffer
    (insert "abcdef")
    ;; start == end: should still create a zero-length overlay
    (let ((ov (rustowl-underline 2 2 "#00ff00")))
      (should (overlayp ov))
      (should (= (overlay-start ov) 2))
      (should (= (overlay-end ov) 2)))
    ;; start > end: should still create an overlay, but Emacs overlays swap start/end
    (let ((ov (rustowl-underline 4 2 "#00ff00")))
      (should (overlayp ov))
      (should (= (overlay-start ov) 2))
      (should (= (overlay-end ov) 4)))
    ;; out-of-bounds: should not error, overlay will be clamped
    (let ((ov (rustowl-underline -5 100 "#00ff00")))
      (should (overlayp ov))
      (should (<= (overlay-start ov) (point-max)))
      (should (>= (overlay-start ov) (point-min)))
      (should (<= (overlay-end ov) (point-max)))
      (should (>= (overlay-end ov) (point-min))))))

;; Test rustowl-clear-overlays when overlays list is empty
(ert-deftest rustowl-test-clear-overlays-empty ()
  (let ((rustowl-overlays nil))
    (should-not (rustowl-clear-overlays))
    (should (null rustowl-overlays))))

;; Test timer logic: rustowl-reset-cursor-timer, rustowl-enable-cursor, rustowl-disable-cursor
(ert-deftest rustowl-test-timer-logic ()
  (let ((rustowl-cursor-timer nil)
        (rustowl-cursor-timeout 0.01)
        (called nil))
    (cl-letf (((symbol-function 'run-with-idle-timer)
               (lambda (_secs _repeat function &rest _args)
                 (setq called t)
                 'fake-timer))
              ((symbol-function 'cancel-timer)
               (lambda (_timer) (setq called 'cancelled)))
              ((symbol-function 'rustowl-clear-overlays)
               (lambda () (setq called 'cleared))))
      (rustowl-reset-cursor-timer)
      (should (or (eq called t) (eq called 'cleared)))
      (setq rustowl-cursor-timer 'fake-timer)
      (rustowl-disable-cursor)
      (should (or (eq called 'cancelled) (eq called 'cleared)))
      (let ((added nil))
        (cl-letf (((symbol-function 'add-hook)
                   (lambda (hook fn) (setq added (list hook fn)))))
          (rustowl-enable-cursor)
          (should
           (equal
            added
            '(post-command-hook rustowl-reset-cursor-timer))))))))

;; Test idempotency of rustowl-enable-cursor and rustowl-disable-cursor
(ert-deftest rustowl-test-enable-disable-idempotent ()
  (let ((add-count 0)
        (remove-count 0))
    (cl-letf (((symbol-function 'add-hook)
               (lambda (hook fn) (cl-incf add-count)))
              ((symbol-function 'remove-hook)
               (lambda (hook fn) (cl-incf remove-count))))
      (rustowl-enable-cursor)
      (rustowl-enable-cursor)
      (should (>= add-count 2))
      (rustowl-disable-cursor)
      (rustowl-disable-cursor)
      (should (>= remove-count 2)))))

;; Test rustowl-reset-cursor-timer when timer is nil
(ert-deftest rustowl-test-reset-cursor-timer-nil ()
  (let ((rustowl-cursor-timer nil)
        (called nil))
    (cl-letf (((symbol-function 'cancel-timer)
               (lambda (_timer) (setq called t)))
              ((symbol-function 'rustowl-clear-overlays)
               (lambda () (setq called 'cleared)))
              ((symbol-function 'run-with-idle-timer)
               (lambda (_timeout _repeat fn &rest _args)
                 'fake-timer)))
      (should (not called))
      (rustowl-reset-cursor-timer)
      (should (not (eq called t)))
      (should (or (eq called 'cleared) (null called))))))

;; Test rustowl-line-col-to-pos with out-of-bounds line/col
(ert-deftest rustowl-test-line-col-to-pos-out-of-bounds ()
  (with-temp-buffer
    (insert "abc\ndef\nghi")
    ;; Negative line/col should signal error
    (should-error (rustowl-line-col-to-pos -1 -1))
    ;; Line past end
    (should (= (rustowl-line-col-to-pos 100 0) (point-max)))
    ;; Col past end of line
    (goto-char (rustowl-line-col-to-pos 0 100))
    (should (>= (point) (point-min)))
    (should (<= (point) (point-max)))))

;; Test rustowl-cursor overlays (mocking lsp-request-async)
(ert-deftest rustowl-test-cursor-overlays ()
  (let ((called nil)
        (response
         (let ((ht (make-hash-table :test 'equal)))
           (let* ((deco1 (make-hash-table :test 'equal))
                  (range1 (make-hash-table :test 'equal))
                  (start1 (make-hash-table :test 'equal))
                  (end1 (make-hash-table :test 'equal))
                  (deco2 (make-hash-table :test 'equal))
                  (range2 (make-hash-table :test 'equal))
                  (start2 (make-hash-table :test 'equal))
                  (end2 (make-hash-table :test 'equal)))
             (puthash "line" 0 start1)
             (puthash "character" 0 start1)
             (puthash "line" 0 end1)
             (puthash "character" 3 end1)
             (puthash "start" start1 range1)
             (puthash "end" end1 range1)
             (puthash "type" "lifetime" deco1)
             (puthash "range" range1 deco1)
             (puthash "overlapped" nil deco1)
             (puthash "line" 0 start2)
             (puthash "character" 4 start2)
             (puthash "line" 0 end2)
             (puthash "character" 6 end2)
             (puthash "start" start2 range2)
             (puthash "end" end2 range2)
             (puthash "type" "imm_borrow" deco2)
             (puthash "range" range2 deco2)
             (puthash "overlapped" nil deco2)
             (puthash "decorations" (vector deco1 deco2) ht)
             ht))))
    (with-temp-buffer
      (insert "abcdef")
      (cl-letf (((symbol-function 'lsp-request-async)
                 (lambda (_method _params cb &rest _args)
                   (funcall cb response)
                   (setq called t)))
                ((symbol-function 'rustowl-underline)
                 (lambda (start end color)
                   (setq called (list start end color))
                   (make-overlay start end))))
        (rustowl-cursor
         '(:position
           (:line 0 :character 0)
           :document (:uri "file:///fake")))
        (should called)))))

;; Test rustowl-cursor overlays for all type branches and overlapped
(ert-deftest rustowl-test-cursor-overlays-all-types ()
  (let ((called-types '()))
    (let* ((make-deco
            (lambda (type &optional overlapped)
              (let ((deco (make-hash-table :test 'equal))
                    (range (make-hash-table :test 'equal))
                    (start (make-hash-table :test 'equal))
                    (end (make-hash-table :test 'equal)))
                (puthash "line" 0 start)
                (puthash "character" 0 start)
                (puthash "line" 0 end)
                (puthash "character" 2 end)
                (puthash "start" start range)
                (puthash "end" end range)
                (puthash "type" type deco)
                (puthash "range" range deco)
                (puthash "overlapped" overlapped deco)
                deco)))
           (response
            (let ((ht (make-hash-table :test 'equal)))
              (puthash
               "decorations"
               (vector
                (funcall make-deco "lifetime")
                (funcall make-deco "imm_borrow")
                (funcall make-deco "mut_borrow")
                (funcall make-deco "move")
                (funcall make-deco "call")
                (funcall make-deco "outlive")
                (funcall make-deco "lifetime" t)) ; overlapped
               ht)
              ht)))
      (with-temp-buffer
        (insert "abcdef")
        (cl-letf (((symbol-function 'lsp-request-async)
                   (lambda (_method _params cb &rest _args)
                     (funcall cb response)))
                  ((symbol-function 'rustowl-underline)
                   (lambda (_start _end color)
                     (push color called-types)
                     (make-overlay 1 2))))
          (rustowl-cursor
           '(:position
             (:line 0 :character 0)
             :document (:uri "file:///fake")))
          ;; Should get all colors except for the overlapped one
          (should (member "#00cc00" called-types)) ; lifetime
          (should (member "#0000cc" called-types)) ; imm_borrow
          (should (member "#cc00cc" called-types)) ; mut_borrow
          (should (member "#cccc00" called-types)) ; move/call
          (should (member "#cc0000" called-types)) ; outlive
          ;; Should not call underline for overlapped
          (should
           (= (length
               (cl-remove-if-not
                (lambda (c) (equal c "#00cc00")) called-types))
              1)))))))

;; Test rustowl-cursor-call (mocking buffer and lsp)
(ert-deftest rustowl-test-cursor-call ()
  (let ((called nil))
    (with-temp-buffer
      (insert "abc\ndef")
      (goto-char (point-min))
      (cl-letf (((symbol-function 'rustowl-line-number-at-pos)
                 (lambda () 0))
                ((symbol-function 'rustowl-current-column)
                 (lambda () 1))
                ((symbol-function 'lsp--buffer-uri)
                 (lambda () "file:///fake"))
                ((symbol-function 'rustowl-cursor)
                 (lambda (params) (setq called params))))
        (rustowl-cursor-call)
        (should
         (equal
          called
          '(:position
            (:line 0 :character 1)
            :document (:uri "file:///fake"))))))))

(provide 'rustowl-test)
;;; rustowl-test.el ends here
