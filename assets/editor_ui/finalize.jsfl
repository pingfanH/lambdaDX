var base='file:///Users/pingfanh/project/lambdaDX/assets/editor_ui/';
var report=base+'animate_validation.txt';
try {
 var d=fl.openDocument(base+'editor_ui.xfl');
 FLfile.write(report,'Opened XFL\n');
 function bitmap(symbol,file,w,h){
  d.library.editItem(symbol);
  d.importFile(base+'source/'+file,false);
  d.selectAll();
  d.setSelectionBounds({left:0,top:0,right:w,bottom:h});
  d.exitEditMode();
 }
 bitmap('UI/Preview/ui_cover','preview_cover.png',347,348);
 bitmap('UI/Tools/ui_shadow','toolbar_shadow.png',54,284);
 fl.saveDocument(d,base+'editor_ui.xfl');
 fl.saveDocument(d,'file:///Users/pingfanh/project/lambdaDX/assets/Editor-1.fla');
 d.exportPNG(base+'preview.png',true,true);
 var s='Saved FLA: assets/Editor-1.fla\nStage: '+d.width+' x '+d.height+'\nLibrary items: '+d.library.items.length+'\n';
 var items=d.library.items;for(var i=0;i<items.length;i++)s+=items[i].name+' | '+items[i].itemType+'\n';
 FLfile.write(report,s);
} catch(e) {FLfile.write(report,'ERROR: '+e);}
