;;; eask-test.el --- Tests for rustowl.el using Eask and ert -*- lexical-binding: t; -*-

(require 'ert)
(require 'rustowl)

(ert-deftest rustowl-line-number-at-pos-test ()
  "Test `rustowl-line-number-at-pos' returns correct line number."
  (with-temp-buffer
    (insert "line1\nline2\nline3")
    (goto-char (point-min))
    (should (= (rustowl-line-number-at-pos) 0))
    (forward-line 1)
    (should (= (rustowl-line-number-at-pos) 1))
    (goto-char (point-max))
    (should (= (rustowl-line-number-at-pos) 2))))

(ert-deftest rustowl-current-column-test ()
  "Test `rustowl-current-column' returns correct column."
  (with-temp-buffer
    (insert "abc\ndef")
    (goto-char (point-min))
    (should (= (rustowl-current-column) 0))
    (forward-char 3)
    (should (= (rustowl-current-column) 3))
    (goto-char (point-max))
    (should (= (rustowl-current-column) 3))))

(ert-deftest rustowl-line-col-to-pos-test ()
  "Test `rustowl-line-col-to-pos' returns correct buffer position."
  (with-temp-buffer
    (insert "abc\ndef\nghi")
    (should (= (rustowl-line-col-to-pos 0 0) (point-min)))
    (should (= (rustowl-line-col-to-pos 1 0)
               (save-excursion (goto-char (point-min)) (forward-line 1) (point))))
    (should (= (rustowl-line-col-to-pos 2 2)
               (save-excursion (goto-char (point-min)) (forward-line 2) (move-to-column 2) (point))))))

(ert-deftest rustowl-underline-and-clear-overlays-test ()
  "Test `rustowl-underline' and `rustowl-clear-overlays'."
  (with-temp-buffer
    (insert "abcde")
    (let ((start (point-min))
          (end (1+ (point-min))))
      (should (= (length rustowl-overlays) 0))
      (let ((ov (rustowl-underline start end "#ff0000")))
        (should (overlayp ov))
        (should (= (length rustowl-overlays) 1))
        (rustowl-clear-overlays)
        (should (= (length rustowl-overlays) 0))))))

(provide 'eask-test)
