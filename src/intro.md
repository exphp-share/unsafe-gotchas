# Introduction

There's plenty of things that you're *obviously* supposed to worry about in `unsafe` code; such as making sure that you don't dereference pointers to invalid data, and that you don't use something after it is freed. But oftentimes there are problems that are not so obvious, and you might forget to think about them even if they are mentioned in the docs of an `unsafe fn`!

This book is a (modest) collection of those unsafe "gotchas."

## About

The source for this book is [hosted on GitHub](https://github.com/exphp-share/unsafe-gotchas). If you'd like to contribute, just submit a PR!
