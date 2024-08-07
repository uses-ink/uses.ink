//go:build wasm

package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"io/fs"
	"log"
	"os"
	"strings"
	"syscall/js"

	"oss.terrastruct.com/d2/d2ast"
	"oss.terrastruct.com/d2/d2compiler"
	"oss.terrastruct.com/d2/d2format"
	"oss.terrastruct.com/d2/d2graph"
	"oss.terrastruct.com/d2/d2layouts/d2dagrelayout"
	"oss.terrastruct.com/d2/d2lib"
	"oss.terrastruct.com/d2/d2oracle"
	"oss.terrastruct.com/d2/d2parser"
	"oss.terrastruct.com/d2/d2renderers/d2svg"
	"oss.terrastruct.com/d2/d2target"
	"oss.terrastruct.com/d2/d2themes"
	"oss.terrastruct.com/d2/d2themes/d2themescatalog"
	"oss.terrastruct.com/d2/lib/textmeasure"
	"oss.terrastruct.com/d2/lib/urlenc"
	"oss.terrastruct.com/util-go/go2"
)

func main() {
	log.SetOutput(io.Discard)

	js.Global().Set("d2GetParentID", js.FuncOf(jsGetParentID))
	js.Global().Set("d2GetObjOrder", js.FuncOf(jsGetObjOrder))
	js.Global().Set("d2GetRefRanges", js.FuncOf(jsGetRefRanges))
	js.Global().Set("d2Compile", js.FuncOf(jsCompile))
	js.Global().Set("d2Parse", js.FuncOf(jsParse))
	js.Global().Set("d2Encode", js.FuncOf(jsEncode))
	js.Global().Set("d2Decode", js.FuncOf(jsDecode))
	js.Global().Set("d2RenderSVG", js.FuncOf(jsRenderSVG))

	initCallback := js.Global().Get("onWasmInitialized")
	if !initCallback.IsUndefined() {
		initCallback.Invoke()
	}
	select {}
}

type jsObjRenderSVG struct {
	SVG       string `json:"svg"`
	Error     string `json:"error"`
	UserError string `json:"userError"`
	D2Error   string `json:"d2Error"`
}

func jsRenderSVG(this js.Value, args []js.Value) interface{} {
	// dsl := args[0].String()
	// dark := args[1].Bool()
	// sketch := args[2].Bool()
	// // Optional
	// var center *bool = nil
	// if len(args) > 3 {
	// 	center = go2.Pointer(args[3].Bool())
	// } else {
	// 	center = go2.Pointer(true)
	// }
	// var padding *int64 = nil
	// if len(args) > 4 {
	// 	padding = go2.Pointer(args[4].Int())
	// } else {
	// 	padding = go2.Pointer(int64(5))
	// }
	obj := args[0]
	if obj.Type() != js.TypeObject {
		ret := jsObjRenderSVG{UserError: "first argument must be an object"}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	dsl := obj.Get("dsl").String()
	dark := obj.Get("dark").Bool()
	sketch := obj.Get("sketch").Bool()
	var center *bool = nil
	if !obj.Get("center").IsUndefined() {
		center = go2.Pointer(obj.Get("center").Bool())
	} else {
		center = go2.Pointer(true)
	}
	var padding *int64 = nil
	if !obj.Get("padding").IsUndefined() {
		padding = go2.Pointer(int64(obj.Get("padding").Int()))
	} else {
		padding = go2.Pointer(int64(5))
	}
	var scale *float64 = nil
	if !obj.Get("scale").IsUndefined() {
		scale = go2.Pointer(obj.Get("scale").Float())
	} else {
		scale = go2.Pointer(1.0)
	}

	ruler, _ := textmeasure.NewRuler()
	layoutResolver := func(engine string) (d2graph.LayoutGraph, error) {
		return d2dagrelayout.DefaultLayout, nil
	}

	var theme *d2themes.Theme
	var overrides *d2target.ThemeOverrides
	if dark {
		theme = &d2themescatalog.DarkFlagshipTerrastruct
		overrides = &d2target.ThemeOverrides{
			B1:  go2.Pointer("#F0F0F0"),
			B2:  go2.Pointer("#989898"),
			B3:  go2.Pointer("#707070"),
			B4:  go2.Pointer("#303030"),
			B5:  go2.Pointer("#212121"),
			B6:  go2.Pointer("#111111"),
			AA2: go2.Pointer("#989898"),
			AA4: go2.Pointer("#303030"),
			AA5: go2.Pointer("#212121"),
			AB4: go2.Pointer("#303030"),
			AB5: go2.Pointer("#212121"),
		}
	} else {
		theme = &d2themescatalog.NeutralGrey
	}

	renderOpts := &d2svg.RenderOpts{
		Pad:            padding,
		ThemeID:        &theme.ID,
		ThemeOverrides: overrides,
		Sketch:         &sketch,
		Center:         center,
		Scale:          scale,
	}

	compileOpts := &d2lib.CompileOptions{
		LayoutResolver: layoutResolver,
		Ruler:          ruler,
	}

	diagram, _, err := d2lib.Compile(context.Background(), dsl, compileOpts, renderOpts)

	if err != nil {
		ret := jsObjRenderSVG{D2Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	out, err := d2svg.Render(diagram, renderOpts)

	if err != nil {
		ret := jsObjRenderSVG{D2Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	ret := jsObjRenderSVG{
		SVG: string(out),
	}
	str, _ := json.Marshal(ret)
	return string(str)
}

type jsObjOrder struct {
	Order []string `json:"order"`
	Error string   `json:"error"`
}

func jsGetObjOrder(this js.Value, args []js.Value) interface{} {
	dsl := args[0].String()

	g, _, err := d2compiler.Compile("", strings.NewReader(dsl), &d2compiler.CompileOptions{
		UTF16Pos: true,
	})
	if err != nil {
		ret := jsObjOrder{Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	objOrder, err := d2oracle.GetObjOrder(g, nil)
	if err != nil {
		ret := jsObjOrder{Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}
	resp := jsObjOrder{
		Order: objOrder,
	}

	str, _ := json.Marshal(resp)
	return string(str)
}

func jsGetParentID(this js.Value, args []js.Value) interface{} {
	id := args[0].String()

	mk, _ := d2parser.ParseMapKey(id)

	if len(mk.Edges) > 0 {
		return ""
	}

	if mk.Key != nil {
		if len(mk.Key.Path) == 1 {
			return "root"
		}
		mk.Key.Path = mk.Key.Path[:len(mk.Key.Path)-1]
		return strings.Join(mk.Key.IDA(), ".")
	}

	return ""
}

type jsRefRanges struct {
	Ranges     []d2ast.Range `json:"ranges"`
	ParseError string        `json:"parseError"`
	UserError  string        `json:"userError"`
	D2Error    string        `json:"d2Error"`
}

func jsGetRefRanges(this js.Value, args []js.Value) interface{} {
	dsl := args[0].String()
	key := args[1].String()

	mk, err := d2parser.ParseMapKey(key)
	if err != nil {
		ret := jsRefRanges{D2Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	g, _, err := d2compiler.Compile("", strings.NewReader(dsl), &d2compiler.CompileOptions{
		UTF16Pos: true,
	})
	var pe *d2parser.ParseError
	if err != nil {
		if errors.As(err, &pe) {
			serialized, _ := json.Marshal(err)
			// TODO
			ret := jsRefRanges{ParseError: string(serialized)}
			str, _ := json.Marshal(ret)
			return string(str)
		}
		ret := jsRefRanges{D2Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	var ranges []d2ast.Range
	if len(mk.Edges) == 1 {
		edge := d2oracle.GetEdge(g, nil, key)
		if edge == nil {
			ret := jsRefRanges{D2Error: "edge not found"}
			str, _ := json.Marshal(ret)
			return string(str)
		}

		for _, ref := range edge.References {
			ranges = append(ranges, ref.MapKey.Range)
		}
	} else {
		obj := d2oracle.GetObj(g, nil, key)
		if obj == nil {
			ret := jsRefRanges{D2Error: "obj not found"}
			str, _ := json.Marshal(ret)
			return string(str)
		}

		for _, ref := range obj.References {
			ranges = append(ranges, ref.Key.Range)
		}
	}

	resp := jsRefRanges{
		Ranges: ranges,
	}

	str, _ := json.Marshal(resp)
	return string(str)
}

type jsObject struct {
	Result    string `json:"result"`
	UserError string `json:"userError"`
	D2Error   string `json:"d2Error"`
}

type jsParseResponse struct {
	DSL        string `json:"dsl"`
	ParseError string `json:"parseError"`
	UserError  string `json:"userError"`
	D2Error    string `json:"d2Error"`
}

type emptyFile struct{}

func (f *emptyFile) Stat() (os.FileInfo, error) {
	return nil, nil
}

func (f *emptyFile) Read(p []byte) (int, error) {
	return 0, io.EOF
}

func (f *emptyFile) Close() error {
	return nil
}

type detectFS struct {
	importUsed bool
}

func (detectFS *detectFS) Open(name string) (fs.File, error) {
	detectFS.importUsed = true
	return &emptyFile{}, nil
}

func jsParse(this js.Value, args []js.Value) interface{} {
	dsl := args[0].String()
	themeID := args[1].Int()

	detectFS := detectFS{}

	g, _, err := d2compiler.Compile("", strings.NewReader(dsl), &d2compiler.CompileOptions{
		UTF16Pos: true,
		FS:       &detectFS,
	})
	// If an import was used, client side D2 cannot reliably compile
	// Defer to backend compilation
	if !detectFS.importUsed {
		var pe *d2parser.ParseError
		if err != nil {
			if errors.As(err, &pe) {
				serialized, _ := json.Marshal(err)
				ret := jsParseResponse{ParseError: string(serialized)}
				str, _ := json.Marshal(ret)
				return string(str)
			}
			ret := jsParseResponse{D2Error: err.Error()}
			str, _ := json.Marshal(ret)
			return string(str)
		}

		for _, o := range g.Objects {
			if (o.Attributes.Top == nil) != (o.Attributes.Left == nil) {
				ret := jsParseResponse{UserError: `keywords "top" and "left" currently must be used together`}
				str, _ := json.Marshal(ret)
				return string(str)
			}
		}

		err = g.ApplyTheme(int64(themeID))
		if err != nil {
			ret := jsParseResponse{D2Error: err.Error()}
			str, _ := json.Marshal(ret)
			return string(str)
		}
	}

	m, err := d2parser.Parse("", strings.NewReader(dsl), &d2parser.ParseOptions{
		UTF16Pos: true,
	})
	if err != nil {
		return err
	}

	resp := jsParseResponse{}

	newDSL := d2format.Format(m)
	if dsl != newDSL {
		resp.DSL = newDSL
	}

	str, _ := json.Marshal(resp)
	return string(str)
}

// TODO error passing
// TODO recover panics
func jsCompile(this js.Value, args []js.Value) interface{} {
	script := args[0].String()

	g, _, err := d2compiler.Compile("", strings.NewReader(script), &d2compiler.CompileOptions{
		UTF16Pos: true,
	})
	var pe *d2parser.ParseError
	if err != nil {
		if errors.As(err, &pe) {
			serialized, _ := json.Marshal(err)
			ret := jsObject{UserError: string(serialized)}
			str, _ := json.Marshal(ret)
			return string(str)
		}
		ret := jsObject{D2Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	newScript := d2format.Format(g.AST)
	if script != newScript {
		ret := jsObject{Result: newScript}
		str, _ := json.Marshal(ret)
		return string(str)
	} else {
		ret := jsObject{Result: script}
		str, _ := json.Marshal(ret)
		return string(str)
	}
}

func jsEncode(this js.Value, args []js.Value) interface{} {
	script := args[0].String()

	encoded, err := urlenc.Encode(script)
	// should never happen
	if err != nil {
		ret := jsObject{D2Error: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	ret := jsObject{Result: encoded}
	str, _ := json.Marshal(ret)
	return string(str)
}

func jsDecode(this js.Value, args []js.Value) interface{} {
	script := args[0].String()

	script, err := urlenc.Decode(script)
	if err != nil {
		ret := jsObject{UserError: err.Error()}
		str, _ := json.Marshal(ret)
		return string(str)
	}

	ret := jsObject{Result: script}
	str, _ := json.Marshal(ret)
	return string(str)
}
