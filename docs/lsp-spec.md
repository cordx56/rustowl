# The RustOwl LSP specification

`rustowl`, is an LSP server which provides RustOwl information.
To display various types of decorations, RustOwl supports some custom methods from the client.

Here, we describe the specifications of those custom methods.

## Types

Here, we describe the types we will use in this document.

### `OprType`

<!-- prettier-ignore-start -->
```typescript
"lifetime" | "imm_borrow" | "mut_borrow" | "move" | "call" | "outlive" | "shared_mut"
```
<!-- prettier-ignore-end -->

### `Decoration`

<pre><code>{
    "type": <a href="#oprtype">OprType</a>,
    "range": <a href="https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#range">Range</a>,
    "hover_text": Option&lt;String&gt;,
    "overlapped": bool
}
</code></pre>

`overlapped` field indicates that the decoration is overlapped and should be hidden.

## Methods

We describe the custom methods used in RustOwl.

### `rustowl/cursor`

#### Request payload

<pre><code>{
    "position": <a href="https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#position">Position</a>,
    "document": {
        "uri": <a href="https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#documentUri">DocumentUri</a>
    }
}
</code></pre>

#### Response payload

<pre><code>{
    "is_analyzed": bool,
    "decorations": [<a href="#decoration">Decoration</a>]
}
</code></pre>
