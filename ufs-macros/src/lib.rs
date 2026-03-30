use proc_macro2::Span;
use quote::quote;
use proc_macro::TokenStream;
use syn::{LitBool, LitStr, Token, parse::{Parse, ParseStream}, parse_macro_input};



struct FsInput {
    path: LitStr,
    read: Option<LitBool>,
    root: Option<LitBool>,
    absolute: Option<LitBool>,
}

impl Parse for FsInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        let mut read = None;
        let mut root = None;
        let mut absolute = None;
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            let ident: syn::Ident = input.parse()?;
            if ident == "read" {
                input.parse::<Token![=]>()?;
                read = Some(input.parse()?);
            } else if ident == "root" {
                input.parse::<Token![=]>()?;
                root = Some(input.parse()?);
            } else if ident == "absolute" {
                input.parse::<Token![=]>()?;
                absolute = Some(input.parse()?);
            } else {
                return Err(syn::Error::new_spanned(
                    ident, 
                    "unknown argument"
                ));
            }
        }
        Ok(FsInput { path, read, root, absolute })
    }
}





/// ### Embed a dir
/// 
/// **commands**
/// + read = **true or false** | (default: true) Read Files
/// + root = **true or false** | (default: false) Read `dir/to/folder` files will be stored with (true) as `folder/file.txt` instead of `file.txt`
/// + absolute = **true or false** | Read Files (default: false) starts from `CARGO_MANIFEST_DIR` or your absolute dir
/// 
/// **usage:** `bind_dir!("dir/to/folder", read = false);`
/// 
#[proc_macro]
pub fn bind_dir(input: TokenStream) -> TokenStream {
    
    let macro_input = parse_macro_input!(input as FsInput);

    let dir_path = macro_input.path.value();
    let with_root = macro_input.root.map(|b| b.value).unwrap_or(false);
    let read_files = macro_input.read.map(|b| b.value).unwrap_or(true);
    let get_absolute_path = macro_input.absolute.map(|b| b.value).unwrap_or(false);

    let full_path = match get_absolute_path {
        true => std::path::PathBuf::from(&dir_path),
        false => std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set")).join(&dir_path),
    };
    
    if !full_path.exists() {
        return syn::Error::new_spanned(dir_path, format!("Directory '{}' not found.", full_path.display())).to_compile_error().into();
    }


    
    let mut files = Vec::new();
    let mut paths = vec![];

    walker(&full_path, &mut paths).unwrap();

    let mut base = full_path.to_str().unwrap().replace('\\', "/");
    
    if with_root {
        base = base.strip_suffix(base.split("/").last().unwrap()).unwrap().into();
    }

    while let Some(pathbuf) = paths.pop() {

        let filename = pathbuf.file_name().map(|s| s.display().to_string()).unwrap();
        let size = pathbuf.metadata().map(|m| m.len()).unwrap_or(0);
        let path = pathbuf.display().to_string().replace('\\', "/");

        let mut fullpath = pathbuf.display().to_string().replace('\\', "/");
        if fullpath.starts_with(&base) { fullpath = fullpath.strip_prefix(&base).unwrap().into(); }
        if fullpath.starts_with("/") { fullpath = fullpath.strip_prefix("/").unwrap().into(); }

        let depth = fullpath.split("/").count() - 1;

        files.push(( path, fullpath, filename, size, depth ));

    }
    


    let vec_entries = files.iter().map(| (filepath, path, name, size, depth) | {
        let fullpath = LitStr::new(path, Span::call_site());
        let filename = LitStr::new(name, Span::call_site());
        let size = *size;
        let depth = *depth;
        quote! {
            ufs::FsEntry {
                size: #size,
                depth: #depth,
                filename: String::from(#filename),
                path: std::path::PathBuf::from(#fullpath),
                content: if #read_files { Some(include_bytes!(#filepath)) } else { None },
            }
        }
    });
    


    let expanded = quote! {
        {
            ufs::UnifiedFS {
                entries: vec![
                    #(#vec_entries),*
                ],
                ..Default::default()
            }
        }
    };
    
    TokenStream::from(expanded)

}


fn walker<P>(path: P, paths: &mut Vec<std::path::PathBuf>) -> std::io::Result<()>
where 
    P: AsRef<std::path::Path>,
{
    let read_dir = std::fs::read_dir(path)?;
    for dir_entry in read_dir.into_iter().filter_map(|e| e.ok()) {
        if dir_entry.metadata()?.is_dir() {
            walker(dir_entry.path(), paths)?;
            continue;
        }  
        paths.push(dir_entry.path().to_path_buf());  
    }
    Ok(())
}