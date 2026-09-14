// expect: ok
(function(){ try { var x = new JavaImporter(); importClass(); return 'ok'; } catch(e) { return 'no:'+e; } })();
